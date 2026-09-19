use tokio::sync::watch;
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct RunStreamSession {
    pub(super) run_id: String,
    pub(super) session_id: Uuid,
    pub(super) cancellation: watch::Receiver<bool>,
}

impl RunStreamSession {
    pub(super) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(super) fn cancellation(&self) -> watch::Receiver<bool> {
        self.cancellation.clone()
    }
}
