use std::{collections::BTreeMap, env, process::Stdio, sync::Arc, time::Duration};

use serde_json::json;
use tauri::{AppHandle, Manager, State};
use tokio::process::Command;
use uuid::Uuid;

use super::{
    debugger::{
        DEBUG_CONTROL_ACK_TIMEOUT, DebugSessionContext, debug_control_channel,
        spawn_debug_control_writer,
    },
    environment::{SAFE_PARENT_ENV, ensure_required_env},
    events::process_started_metadata,
    output::{shared_output_emission_gate, spawn_stream_reader},
    preparation::{prepare_run, revalidate_prepared_run},
    process::{StopSignal, signal_process_tree},
    publish_event,
    state::RuntimeState,
    trace::TraceSession,
};
use crate::{
    domain::{
        DebugControlAction, DebugControlRequest, DebugControlResult, DebugSessionSnapshot,
        EvidenceKind, GetDebugSessionRequest, ListRuntimeEventsRequest, RuntimeEvent,
        StartRunRequest, StartRunResult, StopRunRequest, StopRunResult,
    },
    error::{AoneError, AoneResult},
    state::AppState,
};

const GRACEFUL_STOP_MS: u64 = 1_500;

#[tauri::command]
pub async fn start_run(
    request: StartRunRequest,
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AoneResult<StartRunResult> {
    if request.debug && !request.observe {
        return Err(AoneError::InvalidRequest(
            "cooperative debug mode requires observe=true".into(),
        ));
    }
    let reservation = state.reserve_run_start()?;
    let workspace_root = state.workspace_root()?;
    let workspace = app.state::<AppState>().workspace()?;
    if workspace.root != workspace_root {
        return Err(AoneError::InvalidRequest(
            "runtime and indexed workspace state are not synchronized".into(),
        ));
    }
    let prepared = prepare_run(&request, &state, &workspace_root)?;
    let child_env = state.child_environment(&request.env_names)?;
    let selected_env_names = child_env
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    ensure_required_env(&prepared.required_env, &selected_env_names)?;
    let trace_session = request.observe.then(|| {
        Arc::new(TraceSession {
            nonce: zeroize::Zeroizing::new(Uuid::now_v7().to_string()),
            workspace_id: workspace.id.clone(),
        })
    });
    let debug_session = request.debug.then(|| {
        Arc::new(DebugSessionContext {
            nonce: Arc::new(zeroize::Zeroizing::new(Uuid::now_v7().to_string())),
            workspace_id: workspace.id.clone(),
            debug_session_id: format!("debug:{}", Uuid::now_v7()),
        })
    });
    let mut redaction_values = state.secret_values_for_redaction();
    if let Some(session) = trace_session.as_deref() {
        redaction_values.push(zeroize::Zeroizing::new(session.nonce.to_string()));
    }
    if let Some(session) = debug_session.as_deref() {
        redaction_values.push(zeroize::Zeroizing::new(session.nonce.to_string()));
    }
    let secret_values = Arc::new(redaction_values);

    // The renderer can request only an opaque backend-registered profile ID;
    // clicking Run or Debug is the user authorization, as in other IDEs. Keep
    // the security boundary in Rust by rechecking every validated filesystem
    // object immediately before spawning the backend-owned command.
    revalidate_prepared_run(&prepared)?;

    // Always execute the canonical target. On Unix preserve the original
    // launch spelling as argv[0] for multi-call tools such as rustup proxies.
    let mut command = Command::new(&prepared.executable);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command
            .as_std_mut()
            .arg0(prepared.executable_launch_path.as_os_str());
    }
    command
        .args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdin(if request.debug {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .env_clear();

    for name in SAFE_PARENT_ENV {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    for (name, value) in child_env {
        // `child_environment` rejects every inherited parent-control name and
        // loader hook, so selected project values cannot override this base.
        command.env(name, value);
    }
    if let Some(session) = trace_session.as_deref() {
        command.env("AONE_TRACE_PROTOCOL", "AONE_TRACE_V1");
        command.env("AONE_TRACE_NONCE", session.nonce.as_str());
    }
    if let Some(session) = debug_session.as_deref() {
        command.env("AONE_DEBUG_PROTOCOL", "AONE_DEBUG_V1");
        command.env("AONE_DEBUG_NONCE", session.nonce.as_str());
        command.env("AONE_DEBUG_SESSION_ID", &session.debug_session_id);
    }

    #[cfg(unix)]
    command.process_group(0);

    let mut child = command
        .spawn()
        .map_err(|error| AoneError::Task(format!("failed to start executable: {error}")))?;
    let Some(pid) = child.id() else {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(AoneError::Task("spawned process has no process id".into()));
    };
    let run_id = format!("run:{}", Uuid::now_v7());
    let run_session = reservation.activate(run_id.clone(), pid);

    if let Some(debug_context) = debug_session.as_ref() {
        let Some(stdin) = child.stdin.take() else {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(AoneError::Task(
                "debug process did not expose the reserved control pipe".into(),
            ));
        };
        let (control_sender, control_receiver) = debug_control_channel();
        if state
            .register_debug_session(
                &run_session,
                debug_context.debug_session_id.clone(),
                control_sender,
            )
            .is_err()
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(AoneError::Task(
                "debug session could not be bound to the active process".into(),
            ));
        }
        spawn_debug_control_writer(
            app.clone(),
            run_session.clone(),
            stdin,
            control_receiver,
            Arc::clone(debug_context),
        );
    }

    let mut metadata = process_started_metadata(
        &workspace_root,
        &prepared,
        &selected_env_names,
        secret_values.as_slice(),
        pid,
        request.observe,
        request.debug,
    );
    if request.observe {
        metadata.insert("traceProtocol".into(), json!("AONE_TRACE_V1"));
        metadata.insert("instrumentationRequired".into(), json!(true));
    }
    if let Some(debug_context) = debug_session.as_deref() {
        metadata.insert("debugProtocol".into(), json!("AONE_DEBUG_V1"));
        metadata.insert(
            "debugControlProtocol".into(),
            json!("AONE_DEBUG_CONTROL_V1"),
        );
        metadata.insert(
            "debugSessionId".into(),
            json!(debug_context.debug_session_id),
        );
        metadata.insert("cooperativeSafePointsOnly".into(), json!(true));
        metadata.insert("rawValueInspection".into(), json!(false));
    }
    publish_event(
        &app,
        &state,
        state.next_event(
            Some(run_id.clone()),
            "process.started",
            "Local process started",
            EvidenceKind::Observed,
            Some(run_id.clone()),
            metadata,
        ),
    );

    let output_emission_gate = shared_output_emission_gate();
    if let Some(stdout) = child.stdout.take() {
        spawn_stream_reader(
            app.clone(),
            run_session.clone(),
            "process.stdout",
            "stdout",
            stdout,
            Arc::clone(&secret_values),
            Arc::clone(&output_emission_gate),
            trace_session.clone(),
            debug_session.clone(),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_stream_reader(
            app.clone(),
            run_session,
            "process.stderr",
            "stderr",
            stderr,
            Arc::clone(&secret_values),
            Arc::clone(&output_emission_gate),
            None,
            None,
        );
    }

    let wait_app = app.clone();
    let wait_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = child.wait().await;
        let runtime = wait_app.state::<RuntimeState>();
        match result {
            Ok(status) => {
                let _ = runtime.record_leader_exit_with(&wait_run_id, |active| {
                    let mut metadata = BTreeMap::new();
                    metadata.insert("success".into(), json!(status.success()));
                    metadata.insert("code".into(), json!(status.code()));
                    metadata.insert("stopRequested".into(), json!(active.stopping));
                    let event = runtime.next_event(
                        Some(wait_run_id.clone()),
                        "process.exited",
                        "Local process exited",
                        EvidenceKind::Observed,
                        Some(wait_run_id.clone()),
                        metadata,
                    );
                    publish_event(&wait_app, &runtime, event);
                });
            }
            Err(error) => {
                let recorded = runtime.record_wait_failure_with(&wait_run_id, |_| {
                    let mut metadata = BTreeMap::new();
                    metadata.insert("error".into(), json!(error.to_string()));
                    metadata.insert("processState".into(), json!("unknown"));
                    metadata.insert("supervisionPolicy".into(), json!("terminateThenKill"));
                    let event = runtime.next_event(
                        Some(wait_run_id.clone()),
                        "process.wait_failed",
                        "Could not observe process exit; state is unknown",
                        EvidenceKind::Observed,
                        Some(wait_run_id.clone()),
                        metadata,
                    );
                    publish_event(&wait_app, &runtime, event);
                });
                if recorded
                    .as_ref()
                    .is_some_and(|(active, ())| !active.stopping)
                {
                    // Without a reaper result Aone cannot safely supervise the
                    // managed group. Retain authority, then resolve it through
                    // the same bounded TERM -> KILL policy without claiming an
                    // observed exit or a stopped debugger status.
                    let _ = request_process_stop(&wait_run_id, &wait_app, &runtime, false);
                }
            }
        }
    });

    Ok(StartRunResult { run_id })
}

#[tauri::command]
pub async fn stop_run(
    request: StopRunRequest,
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AoneResult<StopRunResult> {
    let stopped = request_process_stop(&request.run_id, &app, &state, false)?;
    Ok(StopRunResult { stopped })
}

fn request_process_stop(
    run_id: &str,
    app: &AppHandle,
    state: &RuntimeState,
    debug_authority: bool,
) -> AoneResult<bool> {
    let Some(pid) = state.mark_stopping(run_id) else {
        return Ok(false);
    };
    if let Err(error) = signal_process_tree(pid, StopSignal::Terminate) {
        state.cancel_stopping(run_id, pid);
        return Err(error);
    }
    state.confirm_stopping(run_id, pid);
    let mut metadata = BTreeMap::new();
    metadata.insert("pid".into(), json!(pid));
    metadata.insert("debugControlAuthority".into(), json!(debug_authority));
    publish_event(
        app,
        state,
        state.next_event(
            Some(run_id.to_owned()),
            "process.stop_requested",
            "Process stop requested",
            EvidenceKind::Observed,
            Some(run_id.to_owned()),
            metadata,
        ),
    );

    let kill_app = app.clone();
    let kill_run_id = run_id.to_owned();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(GRACEFUL_STOP_MS)).await;
        let runtime = kill_app.state::<RuntimeState>();
        let Some(current_pid) = runtime.stopping_pid(&kill_run_id) else {
            return;
        };
        match signal_process_tree(current_pid, StopSignal::Kill) {
            Ok(()) => runtime.complete_stop_escalation(&kill_run_id, current_pid),
            Err(error) => {
                let mut metadata = BTreeMap::new();
                metadata.insert("pid".into(), json!(current_pid));
                metadata.insert("error".into(), json!(error.to_string()));
                let event = runtime.next_event(
                    Some(kill_run_id.clone()),
                    "process.kill_failed",
                    "Could not complete process-group stop",
                    EvidenceKind::Observed,
                    Some(kill_run_id),
                    metadata,
                );
                publish_event(&kill_app, &runtime, event);
            }
        }
    });

    Ok(true)
}

#[tauri::command]
pub fn list_runtime_events(
    request: ListRuntimeEventsRequest,
    state: State<'_, RuntimeState>,
) -> AoneResult<Vec<RuntimeEvent>> {
    Ok(state.list_events(request.limit))
}

#[tauri::command]
pub fn get_debug_session(
    request: GetDebugSessionRequest,
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AoneResult<DebugSessionSnapshot> {
    ensure_runtime_workspace_current(&app, &state)?;
    state.debug_session(&request.run_id)
}

#[tauri::command]
pub fn control_debug_session(
    request: DebugControlRequest,
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AoneResult<DebugControlResult> {
    ensure_runtime_workspace_current(&app, &state)?;
    if request.action == DebugControlAction::Stop {
        // Process-group stop is authoritative and must not depend on a healthy
        // cooperative protocol, fresh UI epoch, or available stdin channel.
        // It remains bound to the exact current debug session so a stale or
        // fabricated session identity cannot terminate an unrelated run.
        let current = state.debug_session(&request.run_id)?;
        if current.debug_session_id != request.debug_session_id {
            return Err(AoneError::InvalidRequest(
                "debug session identity does not match the active run".into(),
            ));
        }
        let _ = state.queue_debug_control(&request);
        let accepted = request_process_stop(&request.run_id, &app, &state, true)?;
        return Ok(DebugControlResult {
            accepted,
            snapshot: state.debug_session(&request.run_id)?,
        });
    }
    let snapshot = state.queue_debug_control(&request)?;
    if let Some(control_epoch) = snapshot.pending_control_epoch {
        let timeout_app = app.clone();
        let timeout_run_id = request.run_id.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(DEBUG_CONTROL_ACK_TIMEOUT).await;
            timeout_app
                .state::<RuntimeState>()
                .expire_debug_control(&timeout_run_id, control_epoch);
        });
    }
    Ok(DebugControlResult {
        accepted: true,
        snapshot,
    })
}

fn ensure_runtime_workspace_current(app: &AppHandle, state: &RuntimeState) -> AoneResult<()> {
    let runtime_root = state.workspace_root()?;
    let workspace = app.state::<AppState>().workspace()?;
    if workspace.root != runtime_root {
        return Err(AoneError::InvalidRequest(
            "runtime and indexed workspace state are not synchronized".into(),
        ));
    }
    Ok(())
}
