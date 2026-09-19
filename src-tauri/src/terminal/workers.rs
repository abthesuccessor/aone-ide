use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Copy)]
pub(super) enum WorkerKind {
    Writer = 1,
    Reader = 2,
    Waiter = 4,
    Pump = 8,
}

pub(super) struct WorkerLiveness {
    active: AtomicU8,
}

impl WorkerLiveness {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicU8::new(0),
        })
    }

    pub(super) fn enter(self: &Arc<Self>, kind: WorkerKind) -> WorkerGuard {
        self.active.fetch_or(kind as u8, Ordering::AcqRel);
        WorkerGuard {
            state: Arc::clone(self),
            kind,
        }
    }

    #[cfg(test)]
    pub(super) fn active_mask(&self) -> u8 {
        self.active.load(Ordering::Acquire)
    }
}

pub(super) struct WorkerGuard {
    state: Arc<WorkerLiveness>,
    kind: WorkerKind,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.state
            .active
            .fetch_and(!(self.kind as u8), Ordering::AcqRel);
    }
}
