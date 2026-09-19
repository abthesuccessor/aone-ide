use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use tokio::sync::watch;
use uuid::Uuid;

use super::{
    debugger::{DebugControlSender, DebugRegistry},
    environment::validate_args,
    session::RunStreamSession,
    timestamp::utc_timestamp,
    trace_registry::TraceRegistry,
};
use crate::{
    domain::{
        DebugControlRequest, DebugSessionSnapshot, DebugWorkflowEvent, EvidenceKind, RunProfile,
        RuntimeEvent,
    },
    error::{AoneError, AoneResult},
};

mod lifecycle;

const DEFAULT_EVENT_CAPACITY: usize = 2_000;
const MAX_EVENT_LIMIT: usize = 2_000;

#[derive(Debug, Clone)]
pub(super) struct ActiveRun {
    pub(super) pid: u32,
    pub(super) stopping: bool,
    pub(super) leader_exited: bool,
    pub(super) exit_observation_failed: bool,
    pub(super) escalation_complete: bool,
    session_id: Uuid,
    cancellation: watch::Sender<bool>,
    streams_cancelled: bool,
}

#[derive(Debug, Default)]
struct RunRegistry {
    active: HashMap<String, ActiveRun>,
    starting: usize,
    workspace_switching: bool,
}

pub(super) struct RunStartReservation<'a> {
    state: &'a RuntimeState,
    pending: bool,
}

impl RunStartReservation<'_> {
    pub(super) fn activate(mut self, run_id: String, pid: u32) -> RunStreamSession {
        let session_id = Uuid::now_v7();
        let (cancellation, receiver) = watch::channel(false);
        let mut runs = self.state.runs.lock();
        runs.starting = runs.starting.saturating_sub(1);
        runs.active.insert(
            run_id.clone(),
            ActiveRun {
                pid,
                stopping: false,
                leader_exited: false,
                exit_observation_failed: false,
                escalation_complete: false,
                session_id,
                cancellation,
                streams_cancelled: false,
            },
        );
        self.pending = false;
        RunStreamSession {
            run_id,
            session_id,
            cancellation: receiver,
        }
    }
}

impl Drop for RunStartReservation<'_> {
    fn drop(&mut self) {
        if self.pending {
            let mut runs = self.state.runs.lock();
            runs.starting = runs.starting.saturating_sub(1);
        }
    }
}

pub struct RuntimeState {
    workspace_root: RwLock<Option<PathBuf>>,
    pub(super) loaded_env: RwLock<HashMap<String, zeroize::Zeroizing<String>>>,
    profiles: RwLock<HashMap<String, RunProfile>>,
    runs: Mutex<RunRegistry>,
    events: Mutex<VecDeque<RuntimeEvent>>,
    traces: Mutex<TraceRegistry>,
    debugger: Mutex<DebugRegistry>,
    pub(crate) otlp: Mutex<crate::otlp::OtlpReceiverRegistry>,
    event_sequence: AtomicU64,
    event_capacity: usize,
}

impl RuntimeState {
    pub fn new(_app_data_dir: PathBuf) -> Self {
        Self {
            workspace_root: RwLock::new(None),
            loaded_env: RwLock::new(HashMap::new()),
            profiles: RwLock::new(HashMap::new()),
            runs: Mutex::new(RunRegistry::default()),
            events: Mutex::new(VecDeque::new()),
            traces: Mutex::new(TraceRegistry::default()),
            debugger: Mutex::new(DebugRegistry::default()),
            otlp: Mutex::new(crate::otlp::OtlpReceiverRegistry::default()),
            event_sequence: AtomicU64::new(0),
            event_capacity: DEFAULT_EVENT_CAPACITY,
        }
    }

    pub fn begin_workspace_switch(&self) -> AoneResult<()> {
        let mut runs = self.runs.lock();
        if runs.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "a workspace switch is already in progress".into(),
            ));
        }
        if !runs.active.is_empty() || runs.starting > 0 {
            return Err(AoneError::InvalidRequest(
                "stop active local processes before switching workspaces".into(),
            ));
        }
        runs.workspace_switching = true;
        Ok(())
    }

    pub fn cancel_workspace_switch(&self) {
        self.runs.lock().workspace_switching = false;
    }

    /// Installs a canonical root and clears workspace-scoped runtime values.
    pub fn complete_workspace_switch(&self, canonical_root: PathBuf) {
        let mut runs = self.runs.lock();
        *self.workspace_root.write() = Some(canonical_root);
        self.loaded_env.write().clear();
        self.profiles.write().clear();
        self.events.lock().clear();
        self.traces.lock().clear();
        self.debugger.lock().clear();
        self.otlp.lock().stop();
        self.event_sequence.store(0, Ordering::Relaxed);
        runs.workspace_switching = false;
    }

    pub fn workspace_root(&self) -> AoneResult<PathBuf> {
        self.workspace_root
            .read()
            .clone()
            .ok_or(AoneError::WorkspaceNotOpen)
    }

    pub fn replace_env_for_workspace(
        &self,
        expected_root: &Path,
        values: HashMap<String, String>,
    ) -> AoneResult<()> {
        let runs = self.runs.lock();
        self.ensure_current_workspace(expected_root, &runs)?;
        self.replace_env(values)
    }

    pub fn replace_profiles(&self, profiles: Vec<RunProfile>) -> Result<(), String> {
        let mut replacement = HashMap::with_capacity(profiles.len());
        for profile in profiles {
            if profile.id.trim().is_empty() {
                return Err("run profile id cannot be empty".into());
            }
            validate_args(&profile.args).map_err(|error| error.to_string())?;
            if replacement.insert(profile.id.clone(), profile).is_some() {
                return Err("duplicate run profile id".into());
            }
        }
        *self.profiles.write() = replacement;
        Ok(())
    }

    pub fn replace_profiles_for_workspace(
        &self,
        expected_root: &Path,
        profiles: Vec<RunProfile>,
    ) -> Result<(), String> {
        let runs = self.runs.lock();
        self.ensure_current_workspace(expected_root, &runs)
            .map_err(|error| error.to_string())?;
        self.replace_profiles(profiles)
    }

    pub fn record_event(&self, event: RuntimeEvent) {
        let mut events = self.events.lock();
        events.push_back(event);
        while events.len() > self.event_capacity {
            events.pop_front();
        }
    }

    pub(crate) fn delete_trace_events(&self, trace_id: &str) -> AoneResult<usize> {
        if trace_id.is_empty()
            || trace_id.len() > 128
            || !trace_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
        {
            return Err(AoneError::InvalidRequest("invalid trace identifier".into()));
        }
        let runs = self.runs.lock();
        if runs.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "runtime traces cannot be deleted during a workspace switch".into(),
            ));
        }
        let mut events = self.events.lock();
        let before = events.len();
        events.retain(|event| event.trace_id.as_deref() != Some(trace_id));
        let deleted = before.saturating_sub(events.len());
        self.otlp.lock().forget_trace(trace_id);
        Ok(deleted)
    }

    pub fn list_events(&self, limit: Option<usize>) -> Vec<RuntimeEvent> {
        if self.runs.lock().workspace_switching {
            return Vec::new();
        }
        self.list_events_bounded(limit)
    }

    pub fn list_events_for_workspace(
        &self,
        expected_root: &Path,
        limit: Option<usize>,
    ) -> AoneResult<Vec<RuntimeEvent>> {
        let runs = self.runs.lock();
        self.ensure_current_workspace(expected_root, &runs)?;
        Ok(self.list_events_bounded(limit))
    }

    pub fn with_workspace_current<T>(
        &self,
        expected_root: &Path,
        operation: impl FnOnce() -> T,
    ) -> AoneResult<T> {
        let runs = self.runs.lock();
        self.ensure_current_workspace(expected_root, &runs)?;
        Ok(operation())
    }

    fn list_events_bounded(&self, limit: Option<usize>) -> Vec<RuntimeEvent> {
        let events = self.events.lock();
        let requested = limit.unwrap_or(250).clamp(1, MAX_EVENT_LIMIT);
        let skip = events.len().saturating_sub(requested);
        events.iter().skip(skip).cloned().collect()
    }

    pub fn next_event(
        &self,
        run_id: Option<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
        evidence: EvidenceKind,
        trace_id: Option<String>,
        metadata: BTreeMap<String, Value>,
    ) -> RuntimeEvent {
        let sequence = self.event_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        RuntimeEvent {
            id: format!("runtime:{}:{sequence}", Uuid::now_v7()),
            run_id,
            kind: kind.into(),
            timestamp: utc_timestamp(),
            label: label.into(),
            evidence,
            source_node_id: None,
            trace_id,
            metadata,
        }
    }

    pub(super) fn accept_trace_span(
        &self,
        session: &RunStreamSession,
        trace_id: &str,
        span_id: &str,
        parent_span_id: Option<&str>,
        phase: &str,
    ) -> Result<(), &'static str> {
        let runs = self.runs.lock();
        if !session_matches(&runs, session) {
            return Err("run output session is no longer active");
        }
        let mut traces = self.traces.lock();
        traces.accept(&session.run_id, trace_id, span_id, parent_span_id, phase)
    }

    pub(super) fn register_debug_session(
        &self,
        session: &RunStreamSession,
        debug_session_id: String,
        control_sender: DebugControlSender,
    ) -> Result<(), &'static str> {
        let runs = self.runs.lock();
        if !session_matches(&runs, session) {
            return Err("run output session is no longer active");
        }
        self.debugger.lock().start(
            session.run_id().to_owned(),
            debug_session_id,
            control_sender,
        );
        Ok(())
    }

    pub(super) fn accept_debug_event(
        &self,
        session: &RunStreamSession,
        event: DebugWorkflowEvent,
    ) -> Result<(), &'static str> {
        let runs = self.runs.lock();
        if !session_matches(&runs, session) {
            return Err("run output session is no longer active");
        }
        self.debugger.lock().accept(session.run_id(), event)
    }

    pub fn debug_session(&self, run_id: &str) -> AoneResult<DebugSessionSnapshot> {
        if self.runs.lock().workspace_switching {
            return Err(AoneError::InvalidRequest(
                "debug session cannot be read during a workspace switch".into(),
            ));
        }
        self.debugger.lock().snapshot(run_id)
    }

    pub(super) fn queue_debug_control(
        &self,
        request: &DebugControlRequest,
    ) -> AoneResult<DebugSessionSnapshot> {
        let runs = self.runs.lock();
        if runs.workspace_switching || !runs.active.contains_key(&request.run_id) {
            return Err(AoneError::InvalidRequest(
                "debug control is not bound to the current active run".into(),
            ));
        }
        self.debugger.lock().queue_control(request)
    }

    pub(super) fn fail_debug_control(&self, run_id: &str, message: String) {
        self.debugger.lock().control_failed(run_id, message);
    }

    pub(super) fn expire_debug_control(&self, run_id: &str, control_epoch: u64) {
        self.debugger.lock().expire_control(run_id, control_epoch);
    }

    fn clear_trace_run(&self, run_id: &str) {
        self.traces.lock().clear_run(run_id);
    }

    pub(super) fn run_session_is_current(&self, session: &RunStreamSession) -> bool {
        session_matches(&self.runs.lock(), session)
    }

    pub(super) fn with_current_run_session<T>(
        &self,
        session: &RunStreamSession,
        operation: impl FnOnce() -> T,
    ) -> Option<T> {
        let runs = self.runs.lock();
        session_matches(&runs, session).then(operation)
    }

    pub(super) fn registered_profile(&self, id: &str) -> Option<RunProfile> {
        self.profiles.read().get(id).cloned()
    }

    pub(super) fn reserve_run_start(&self) -> AoneResult<RunStartReservation<'_>> {
        let mut runs = self.runs.lock();
        if runs.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "cannot start a process while switching workspaces".into(),
            ));
        }
        if runs.starting > 0 || !runs.active.is_empty() {
            return Err(AoneError::InvalidRequest(
                "another local process is already starting or running".into(),
            ));
        }
        runs.starting += 1;
        Ok(RunStartReservation {
            state: self,
            pending: true,
        })
    }

    fn ensure_current_workspace(&self, expected_root: &Path, runs: &RunRegistry) -> AoneResult<()> {
        if runs.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "workspace-scoped runtime state cannot change during a workspace switch".into(),
            ));
        }
        let current = self.workspace_root.read();
        if current.as_deref() != Some(expected_root) {
            return Err(AoneError::InvalidRequest(
                "workspace changed before runtime state could be installed".into(),
            ));
        }
        Ok(())
    }
}

fn session_matches(runs: &RunRegistry, session: &RunStreamSession) -> bool {
    !runs.workspace_switching
        && runs.active.get(&session.run_id).is_some_and(|run| {
            run.session_id == session.session_id && !run.leader_exited && !run.streams_cancelled
        })
}
