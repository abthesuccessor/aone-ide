use std::net::{Ipv4Addr, SocketAddr};

use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tokio::{
    net::TcpListener,
    sync::{oneshot, watch},
};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{
    http::spawn_receiver,
    state::{DEFAULT_OTLP_PORT, OTLP_AUTH_HEADER, OTLP_PROTOCOL},
};
use crate::{
    domain::{
        DeleteRuntimeTraceRequest, DeleteRuntimeTraceResult, OtlpReceiverSnapshot,
        StartOtlpReceiverRequest,
    },
    error::{AoneError, AoneResult},
    runner::{RuntimeState, timestamp::utc_timestamp},
    state::{AppState, WorkspaceContext},
};

#[tauri::command]
pub async fn start_otlp_receiver(
    request: StartOtlpReceiverRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<OtlpReceiverSnapshot> {
    let workspace = app_state.workspace()?;
    runtime.otlp.lock().reserve_start(&workspace.id)?;
    let result = start_reserved_receiver(request, &app, &app_state, &runtime, &workspace).await;
    if result.is_err() {
        runtime.otlp.lock().cancel_start();
    }
    result
}

async fn start_reserved_receiver(
    request: StartOtlpReceiverRequest,
    app: &AppHandle,
    app_state: &AppState,
    runtime: &RuntimeState,
    workspace: &WorkspaceContext,
) -> AoneResult<OtlpReceiverSnapshot> {
    let workspace_id = workspace.id.as_str();
    let port = request.preferred_port.unwrap_or(DEFAULT_OTLP_PORT);
    if port != 0 && port < 1_024 {
        return Err(AoneError::InvalidRequest(
            "the OTLP receiver port must be 0 or between 1024 and 65535".into(),
        ));
    }
    confirm_receiver_start(app, port).await?;
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
        .await
        .map_err(|error| {
            AoneError::Task(format!(
                "could not bind the loopback OTLP receiver: {error}"
            ))
        })?;
    let local_port = listener.local_addr()?.port();
    let endpoint = format!("http://127.0.0.1:{local_port}/v1/traces");
    let receiver_id = format!("otlp:{}", Uuid::now_v7());
    let token = Zeroizing::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
    ));
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    app_state.while_workspace_current(workspace_id, || {
        runtime.with_workspace_current(&workspace.root, || {
            runtime.otlp.lock().activate(
                workspace_id,
                receiver_id.clone(),
                endpoint,
                token.clone(),
                shutdown_sender,
                utc_timestamp(),
            )
        })
    })??;
    spawn_receiver(
        app.clone(),
        listener,
        receiver_id,
        workspace_id.to_owned(),
        shutdown_receiver,
    );
    Ok(runtime.otlp.lock().snapshot())
}

#[tauri::command]
pub fn get_otlp_receiver(runtime: State<'_, RuntimeState>) -> OtlpReceiverSnapshot {
    runtime.otlp.lock().snapshot()
}

#[tauri::command]
pub fn stop_otlp_receiver(runtime: State<'_, RuntimeState>) -> OtlpReceiverSnapshot {
    runtime.otlp.lock().stop();
    runtime.otlp.lock().snapshot()
}

#[tauri::command]
pub fn delete_runtime_trace(
    request: DeleteRuntimeTraceRequest,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<DeleteRuntimeTraceResult> {
    Ok(DeleteRuntimeTraceResult {
        deleted_events: runtime.delete_trace_events(&request.trace_id)?,
    })
}

async fn confirm_receiver_start(app: &AppHandle, port: u16) -> AoneResult<()> {
    let port_label = if port == 0 {
        "a system-selected loopback port".to_string()
    } else {
        format!("127.0.0.1:{port}")
    };
    let message = format!(
        "Aone IDE is ready to open a local OpenTelemetry trace receiver on {port_label}.\n\nOnly OTLP/HTTP JSON POST requests to /v1/traces are accepted. Every request requires an ephemeral {OTLP_AUTH_HEADER} token. The receiver keeps a bounded in-memory trace tail for the current workspace and retains no raw attribute values, span events, links, request bodies, database statements, or payloads.\n\nProtocol: {OTLP_PROTOCOL}. Protobuf, gRPC, metrics, logs, and profiles remain unsupported."
    );
    let (sender, receiver) = oneshot::channel();
    app.dialog()
        .message(message)
        .title("Start local OTLP trace receiver")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Start receiver".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "local OTLP receiver start was cancelled".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "OTLP receiver confirmation dialog closed unexpectedly".into(),
        )),
    }
}
