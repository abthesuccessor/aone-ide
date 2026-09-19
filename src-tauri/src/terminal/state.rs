use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use parking_lot::Mutex;
use portable_pty::{ChildKiller, MasterPty, PtySize};

use super::{
    input::{InputQueue, TerminalWriter},
    workers::WorkerLiveness,
};
use crate::error::{AoneError, AoneResult};

type TerminalMaster = Box<dyn MasterPty + Send>;
type TerminalKiller = Box<dyn ChildKiller + Send + Sync>;

pub(super) struct TerminalSession {
    id: String,
    input: Mutex<Option<InputQueue>>,
    master: Mutex<Option<TerminalMaster>>,
    killer: Mutex<Option<TerminalKiller>>,
    process_group: Option<i32>,
    closed: Arc<AtomicBool>,
    workers: Arc<WorkerLiveness>,
}

impl TerminalSession {
    pub(super) fn new(
        id: String,
        writer: TerminalWriter,
        master: TerminalMaster,
        killer: TerminalKiller,
        process_group: Option<i32>,
        registry: Weak<TerminalRegistry>,
    ) -> Self {
        let closed = Arc::new(AtomicBool::new(false));
        let workers = WorkerLiveness::new();
        let failure_session_id = id.clone();
        let input = InputQueue::start(
            writer,
            Arc::clone(&closed),
            Arc::clone(&workers),
            Box::new(move || {
                if let Some(registry) = registry.upgrade() {
                    registry.close(&failure_session_id);
                }
            }),
        );
        Self {
            id,
            input: Mutex::new(Some(input)),
            master: Mutex::new(Some(master)),
            killer: Mutex::new(Some(killer)),
            process_group,
            closed,
            workers,
        }
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn closed_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.closed)
    }

    pub(super) fn workers(&self) -> Arc<WorkerLiveness> {
        Arc::clone(&self.workers)
    }

    pub(super) fn write(&self, data: Vec<u8>) -> AoneResult<bool> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        let input = self.input.lock();
        if self.closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        match input.as_ref() {
            Some(input) => input.try_enqueue(data),
            None => Ok(false),
        }
    }

    pub(super) fn resize(&self, size: PtySize) -> AoneResult<bool> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        let master = self.master.lock();
        let Some(master) = master.as_ref() else {
            return Ok(false);
        };
        master
            .resize(size)
            .map(|_| true)
            .map_err(|error| AoneError::Task(format!("failed to resize terminal: {error}")))
    }

    pub(super) fn shutdown(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.input.lock().take();
        signal_process_group(self.process_group);
        if let Some(mut killer) = self.killer.lock().take() {
            let _ = killer.kill();
        }
        force_kill_process_group(self.process_group);
        self.master.lock().take();
    }

    fn complete(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.input.lock().take();
        self.killer.lock().take();
        self.master.lock().take();
    }

    #[cfg(test)]
    pub(super) fn input_connected(&self) -> bool {
        self.input
            .lock()
            .as_ref()
            .is_some_and(InputQueue::is_connected)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

enum Slot {
    Idle,
    Starting(u64),
    Active(Arc<TerminalSession>),
}

struct RegistryState {
    generation: u64,
    workspace_switching: bool,
    slot: Slot,
}

pub(super) struct TerminalRegistry {
    state: Mutex<RegistryState>,
}

pub(super) struct TerminalOpenReservation {
    registry: Arc<TerminalRegistry>,
    generation: u64,
    pending: bool,
}

impl TerminalOpenReservation {
    pub(super) fn activate(mut self, session: Arc<TerminalSession>) -> AoneResult<()> {
        let mut state = self.registry.state.lock();
        match state.slot {
            Slot::Starting(generation) if generation == self.generation => {
                state.slot = Slot::Active(session);
                self.pending = false;
                Ok(())
            }
            _ => {
                drop(state);
                session.shutdown();
                Err(AoneError::InvalidRequest(
                    "terminal open was cancelled before activation".into(),
                ))
            }
        }
    }
}

impl Drop for TerminalOpenReservation {
    fn drop(&mut self) {
        if !self.pending {
            return;
        }
        let mut state = self.registry.state.lock();
        if matches!(state.slot, Slot::Starting(generation) if generation == self.generation) {
            state.slot = Slot::Idle;
        }
    }
}

impl TerminalRegistry {
    fn new() -> Self {
        Self {
            state: Mutex::new(RegistryState {
                generation: 0,
                workspace_switching: false,
                slot: Slot::Idle,
            }),
        }
    }

    pub(super) fn reserve(self: &Arc<Self>) -> AoneResult<TerminalOpenReservation> {
        let mut state = self.state.lock();
        if state.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "cannot open a terminal while switching workspaces".into(),
            ));
        }
        if !matches!(state.slot, Slot::Idle) {
            return Err(AoneError::InvalidRequest(
                "another terminal is already starting or open".into(),
            ));
        }
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.slot = Slot::Starting(generation);
        Ok(TerminalOpenReservation {
            registry: Arc::clone(self),
            generation,
            pending: true,
        })
    }

    pub(super) fn session(&self, id: &str) -> Option<Arc<TerminalSession>> {
        let state = self.state.lock();
        match &state.slot {
            Slot::Active(session) if session.id() == id => Some(Arc::clone(session)),
            _ => None,
        }
    }

    pub(super) fn close(&self, id: &str) -> bool {
        let session = {
            let mut state = self.state.lock();
            let matches = matches!(&state.slot, Slot::Active(session) if session.id() == id);
            if !matches {
                return false;
            }
            match std::mem::replace(&mut state.slot, Slot::Idle) {
                Slot::Active(session) => session,
                _ => unreachable!("terminal slot was checked while locked"),
            }
        };
        session.shutdown();
        true
    }

    pub(super) fn finish(&self, id: &str) {
        let session = {
            let mut state = self.state.lock();
            let matches = matches!(&state.slot, Slot::Active(session) if session.id() == id);
            if !matches {
                return;
            }
            match std::mem::replace(&mut state.slot, Slot::Idle) {
                Slot::Active(session) => session,
                _ => unreachable!("terminal slot was checked while locked"),
            }
        };
        session.complete();
    }

    fn begin_workspace_switch(&self) -> AoneResult<()> {
        let mut state = self.state.lock();
        if state.workspace_switching {
            return Err(AoneError::InvalidRequest(
                "a terminal workspace switch is already in progress".into(),
            ));
        }
        if !matches!(state.slot, Slot::Idle) {
            return Err(AoneError::InvalidRequest(
                "close the active terminal before switching workspaces".into(),
            ));
        }
        state.workspace_switching = true;
        Ok(())
    }

    fn complete_workspace_switch(&self) {
        self.state.lock().workspace_switching = false;
    }

    fn close_all(&self) -> bool {
        let previous = {
            let mut state = self.state.lock();
            state.generation = state.generation.wrapping_add(1);
            std::mem::replace(&mut state.slot, Slot::Idle)
        };
        match previous {
            Slot::Idle => false,
            Slot::Starting(_) => true,
            Slot::Active(session) => {
                session.shutdown();
                true
            }
        }
    }
}

pub struct TerminalState {
    registry: Arc<TerminalRegistry>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(TerminalRegistry::new()),
        }
    }

    pub fn close_all(&self) -> bool {
        self.registry.close_all()
    }

    pub fn begin_workspace_switch(&self) -> AoneResult<()> {
        self.registry.begin_workspace_switch()
    }

    pub fn cancel_workspace_switch(&self) {
        self.registry.complete_workspace_switch();
    }

    pub fn complete_workspace_switch(&self) {
        self.registry.complete_workspace_switch();
    }

    pub(super) fn registry(&self) -> Arc<TerminalRegistry> {
        Arc::clone(&self.registry)
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TerminalState {
    fn drop(&mut self) {
        self.close_all();
    }
}

#[cfg(unix)]
fn signal_process_group(process_group: Option<i32>) {
    let Some(process_group) = process_group.filter(|value| *value > 0) else {
        return;
    };
    let _ = unsafe { libc::kill(-process_group, libc::SIGHUP) };
}

#[cfg(unix)]
fn force_kill_process_group(process_group: Option<i32>) {
    let Some(process_group) = process_group.filter(|value| *value > 0) else {
        return;
    };
    let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
}

#[cfg(not(unix))]
fn signal_process_group(_process_group: Option<i32>) {}

#[cfg(not(unix))]
fn force_kill_process_group(_process_group: Option<i32>) {}
