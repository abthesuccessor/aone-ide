use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    domain::{
        DebugActionCapability, DebugCapabilities, DebugControlAction, DebugControlRequest,
        DebugSafePointState, DebugSessionSnapshot, DebugSessionStatus, DebugSourceLocation,
        DebugWorkflowEvent,
    },
    error::{AoneError, AoneResult},
};

use super::{DebugControlSender, QueuedDebugControl};
use crate::runner::timestamp::utc_timestamp;

pub(super) const MAX_DEBUG_EVENTS_PER_SESSION: usize = 2_000;
const MAX_DEBUG_SNAPSHOT_EVENTS: usize = 250;
const MAX_RETAINED_DEBUG_SESSIONS: usize = 8;

#[derive(Default)]
pub(crate) struct DebugRegistry {
    sessions: HashMap<String, DebugSessionState>,
    order: VecDeque<String>,
}

struct DebugSessionState {
    debug_session_id: String,
    run_id: String,
    status: DebugSessionStatus,
    events: Vec<DebugWorkflowEvent>,
    workflow_steps: HashSet<(String, String)>,
    current_step_id: Option<String>,
    current_source: Option<DebugSourceLocation>,
    control_epoch: u64,
    acknowledged_control_epoch: u64,
    pending_action: Option<DebugControlAction>,
    pending_control_epoch: Option<u64>,
    active_workflow_id: Option<String>,
    workflow_count: usize,
    last_workflow_state: Option<DebugSafePointState>,
    started_at: String,
    ended_at: Option<String>,
    error: Option<String>,
    control_sender: DebugControlSender,
}

impl DebugRegistry {
    pub(crate) fn start(
        &mut self,
        run_id: String,
        debug_session_id: String,
        control_sender: DebugControlSender,
    ) {
        while self.order.len() >= MAX_RETAINED_DEBUG_SESSIONS {
            if let Some(oldest) = self.order.pop_front() {
                self.sessions.remove(&oldest);
            }
        }
        self.order.retain(|stored| stored != &run_id);
        self.order.push_back(run_id.clone());
        self.sessions.insert(
            run_id.clone(),
            DebugSessionState {
                debug_session_id,
                run_id,
                status: DebugSessionStatus::Starting,
                events: Vec::new(),
                workflow_steps: HashSet::new(),
                current_step_id: None,
                current_source: None,
                control_epoch: 0,
                acknowledged_control_epoch: 0,
                pending_action: None,
                pending_control_epoch: None,
                active_workflow_id: None,
                workflow_count: 0,
                last_workflow_state: None,
                started_at: utc_timestamp(),
                ended_at: None,
                error: None,
                control_sender,
            },
        );
    }

    pub(crate) fn accept(
        &mut self,
        run_id: &str,
        event: DebugWorkflowEvent,
    ) -> Result<(), &'static str> {
        let Some(session) = self.sessions.get_mut(run_id) else {
            return Err("debug session is not registered for this run");
        };
        if terminal_status(session.status) {
            return Err("debug session already reached a terminal state");
        }
        if session.events.len() >= MAX_DEBUG_EVENTS_PER_SESSION {
            return Err("debug event capacity reached for this session");
        }
        let expected_sequence = session
            .events
            .last()
            .map_or(1, |stored| stored.sequence + 1);
        if event.sequence != expected_sequence {
            return Err("debug safe-point sequence is stale or out of order");
        }
        let acknowledging_control = session.pending_control_epoch == Some(event.control_epoch);
        let in_flight_before_cutover = session.pending_action.is_some()
            && event.control_epoch == session.acknowledged_control_epoch
            && matches!(
                session.pending_action,
                Some(DebugControlAction::Pause | DebugControlAction::Stop)
            );
        if event.control_epoch != session.acknowledged_control_epoch
            && !acknowledging_control
            && !in_flight_before_cutover
        {
            return Err("debug safe point belongs to a stale control epoch");
        }
        let step_key = (event.workflow_id.clone(), event.step_id.clone());
        if session.workflow_steps.contains(&step_key) {
            return Err("duplicate debug step identity");
        }
        if event.parent_step_id.as_ref().is_some_and(|parent| {
            !session
                .workflow_steps
                .contains(&(event.workflow_id.clone(), parent.clone()))
        }) {
            return Err("debug parent step has not been accepted in this session");
        }
        let starts_workflow = session.active_workflow_id.is_none();
        if session
            .active_workflow_id
            .as_ref()
            .is_some_and(|workflow| workflow != &event.workflow_id)
        {
            return Err("concurrent cooperative debug workflows are not supported");
        }
        if starts_workflow && event.safe_point_state != DebugSafePointState::Paused {
            return Err("a cooperative debug workflow must begin at an acknowledged pause");
        }
        if session.status == DebugSessionStatus::Paused && session.pending_action.is_none() {
            return Err("paused debug session advanced without a control permit");
        }
        if let Some(action) = session.pending_action
            && acknowledging_control
        {
            let acknowledged = match action {
                DebugControlAction::Pause => matches!(
                    event.safe_point_state,
                    DebugSafePointState::Paused
                        | DebugSafePointState::WorkflowCompleted
                        | DebugSafePointState::WorkflowFailed
                ),
                DebugControlAction::Resume => true,
                DebugControlAction::StepInto | DebugControlAction::StepOver => matches!(
                    event.safe_point_state,
                    DebugSafePointState::Paused
                        | DebugSafePointState::WorkflowCompleted
                        | DebugSafePointState::WorkflowFailed
                ),
                DebugControlAction::Stop => true,
            };
            if !acknowledged {
                return Err("debug safe point did not acknowledge the pending control action");
            }
            session.acknowledged_control_epoch = event.control_epoch;
            session.pending_action = None;
            session.pending_control_epoch = None;
        }
        if starts_workflow {
            session.active_workflow_id = Some(event.workflow_id.clone());
            session.workflow_count = session.workflow_count.saturating_add(1);
        }
        session.workflow_steps.insert(step_key);
        session.status = match event.safe_point_state {
            DebugSafePointState::Running => DebugSessionStatus::Running,
            DebugSafePointState::Paused => DebugSessionStatus::Paused,
            DebugSafePointState::WorkflowCompleted | DebugSafePointState::WorkflowFailed => {
                session.active_workflow_id = None;
                session.last_workflow_state = Some(event.safe_point_state);
                DebugSessionStatus::Running
            }
        };
        session.current_step_id = Some(event.step_id.clone());
        session.current_source = Some(event.source.clone());
        session.events.push(event);
        Ok(())
    }

    pub(crate) fn snapshot(&self, run_id: &str) -> AoneResult<DebugSessionSnapshot> {
        self.sessions
            .get(run_id)
            .map(DebugSessionState::snapshot)
            .ok_or_else(|| {
                AoneError::InvalidRequest(
                    "no cooperative debug session exists for the requested run".into(),
                )
            })
    }

    pub(crate) fn queue_control(
        &mut self,
        request: &DebugControlRequest,
    ) -> AoneResult<DebugSessionSnapshot> {
        let Some(session) = self.sessions.get_mut(&request.run_id) else {
            return Err(AoneError::InvalidRequest(
                "no cooperative debug session exists for the requested run".into(),
            ));
        };
        if session.debug_session_id != request.debug_session_id {
            return Err(AoneError::InvalidRequest(
                "debug session identity does not match the active run".into(),
            ));
        }
        let current_sequence = session.events.last().map_or(0, |event| event.sequence);
        let sequence_is_fresh = match request.action {
            DebugControlAction::Pause | DebugControlAction::Stop => {
                request.expected_sequence <= current_sequence
            }
            DebugControlAction::Resume
            | DebugControlAction::StepInto
            | DebugControlAction::StepOver => request.expected_sequence == current_sequence,
        };
        if !sequence_is_fresh || request.expected_control_epoch != session.control_epoch {
            return Err(AoneError::InvalidRequest(
                "debug control is stale; refresh the session before controlling it".into(),
            ));
        }
        if terminal_status(session.status) {
            return Err(AoneError::InvalidRequest(
                "debug session already reached a terminal state".into(),
            ));
        }
        if session.pending_action.is_some() {
            return Err(AoneError::InvalidRequest(
                "a cooperative debug control is already awaiting acknowledgement".into(),
            ));
        }
        let allowed = match request.action {
            DebugControlAction::Pause => session.status == DebugSessionStatus::Running,
            DebugControlAction::Resume
            | DebugControlAction::StepInto
            | DebugControlAction::StepOver => session.status == DebugSessionStatus::Paused,
            DebugControlAction::Stop => matches!(
                session.status,
                DebugSessionStatus::Starting
                    | DebugSessionStatus::Running
                    | DebugSessionStatus::Paused
            ),
        };
        if !allowed {
            return Err(AoneError::InvalidRequest(format!(
                "debug action {:?} is not available while the session is {:?}",
                request.action, session.status
            )));
        }

        let next_epoch = session.control_epoch.saturating_add(1);
        session
            .control_sender
            .try_send(QueuedDebugControl {
                action: request.action,
                control_epoch: next_epoch,
                expected_sequence: current_sequence,
            })
            .map_err(|error| {
                AoneError::Task(format!("debug control channel is unavailable: {error}"))
            })?;
        session.control_epoch = next_epoch;
        session.pending_action = Some(request.action);
        session.pending_control_epoch = Some(next_epoch);
        Ok(session.snapshot())
    }

    pub(crate) fn process_exited(&mut self, run_id: &str, stopped: bool) {
        let Some(session) = self.sessions.get_mut(run_id) else {
            return;
        };
        if terminal_status(session.status) {
            session.pending_action = None;
            session.pending_control_epoch = None;
            return;
        }
        session.status = if stopped {
            DebugSessionStatus::Stopped
        } else if session.events.is_empty() || session.active_workflow_id.is_some() {
            DebugSessionStatus::Failed
        } else {
            DebugSessionStatus::Completed
        };
        session.pending_action = None;
        session.pending_control_epoch = None;
        session.ended_at = Some(utc_timestamp());
        session.error = (session.status == DebugSessionStatus::Failed).then(|| {
            if session.events.is_empty() {
                "managed child exited without acknowledging any AONE_DEBUG_V1 safe point".into()
            } else {
                "managed child exited before a terminal AONE_DEBUG_V1 acknowledgement".into()
            }
        });
    }

    pub(crate) fn control_failed(&mut self, run_id: &str, message: String) {
        if let Some(session) = self.sessions.get_mut(run_id) {
            session.status = DebugSessionStatus::Failed;
            session.pending_action = None;
            session.pending_control_epoch = None;
            session.ended_at = Some(utc_timestamp());
            session.error = Some(message);
        }
    }

    pub(crate) fn expire_control(&mut self, run_id: &str, control_epoch: u64) {
        let Some(session) = self.sessions.get_mut(run_id) else {
            return;
        };
        if !terminal_status(session.status) && session.pending_control_epoch == Some(control_epoch)
        {
            session.status = DebugSessionStatus::Failed;
            session.pending_action = None;
            session.pending_control_epoch = None;
            session.ended_at = Some(utc_timestamp());
            session.error = Some(
                "instrumented child did not acknowledge the cooperative control within 30 seconds; the debug protocol session is terminal, but process execution state was not inferred"
                    .into(),
            );
        }
    }

    pub(crate) fn clear(&mut self) {
        self.sessions.clear();
        self.order.clear();
    }
}

impl DebugSessionState {
    fn snapshot(&self) -> DebugSessionSnapshot {
        let skip = self.events.len().saturating_sub(MAX_DEBUG_SNAPSHOT_EVENTS);
        let events = self.events.iter().skip(skip).cloned().collect::<Vec<_>>();
        DebugSessionSnapshot {
            debug_session_id: self.debug_session_id.clone(),
            run_id: self.run_id.clone(),
            status: self.status,
            event_count: self.events.len(),
            control_epoch: self.control_epoch,
            acknowledged_control_epoch: self.acknowledged_control_epoch,
            current_step_id: self.current_step_id.clone(),
            current_source: self.current_source.clone(),
            pending_action: self.pending_action,
            pending_control_epoch: self.pending_control_epoch,
            active_workflow_id: self.active_workflow_id.clone(),
            workflow_count: self.workflow_count,
            last_workflow_state: self.last_workflow_state,
            started_at: self.started_at.clone(),
            ended_at: self.ended_at.clone(),
            error: self.error.clone(),
            capabilities: capabilities(),
            limitation: "Cooperative safe points from an explicitly instrumented managed child. Pause occurs only when that child acknowledges its next safe point; stepping is an instrumentation hint, not OS suspension, DAP stack stepping, browser interception, or arbitrary uninstrumented line execution. There are no dedicated raw-value/query fields, but reported identifiers may disclose schema and Aone cannot independently prove they contain no values or personal data.".into(),
            events_start_sequence: events.first().map(|event| event.sequence),
            events_truncated: skip > 0,
            events,
        }
    }
}

fn terminal_status(status: DebugSessionStatus) -> bool {
    matches!(
        status,
        DebugSessionStatus::Completed | DebugSessionStatus::Failed | DebugSessionStatus::Stopped
    )
}

fn capabilities() -> DebugCapabilities {
    let cooperative = |limitation: &str| DebugActionCapability {
        supported: true,
        cooperative: true,
        limitation: Some(limitation.into()),
    };
    DebugCapabilities {
        pause: cooperative(
            "Takes effect only when the instrumented child acknowledges its next safe point; other threads or requests may continue.",
        ),
        resume: cooperative("Continues the instrumented workflow after an acknowledged pause."),
        step_into: cooperative(
            "Releases one instrumented safe-point permit; call-stack entry depends on the child instrumentation.",
        ),
        step_over: cooperative(
            "Releases one instrumented safe-point permit; this is not a DAP frame-aware step-over.",
        ),
        stop: DebugActionCapability {
            supported: true,
            cooperative: false,
            limitation: Some(
                "Uses the managed-run process-group TERM then KILL authority; a cooperative stdin request is best-effort only, and stopped is reported after observed leader exit."
                    .into(),
            ),
        },
    }
}
