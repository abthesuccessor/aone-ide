use std::sync::Arc;

use dashmap::{DashMap, mapref::entry::Entry};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{AoneError, AoneResult};

pub(super) const MAX_ACTIVE_SESSIONS: usize = 8;
pub(super) const OUTBOUND_QUEUE_CAPACITY: usize = 8;

#[derive(Debug)]
pub(super) struct SessionControl {
    pub(super) outbound: mpsc::Sender<Message>,
    pub(super) shutdown: watch::Sender<bool>,
}

#[derive(Debug)]
pub(super) struct SessionRegistry {
    sessions: DashMap<String, SessionControl>,
    slots: Arc<Semaphore>,
}

impl SessionRegistry {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            sessions: DashMap::with_capacity(capacity),
            slots: Arc::new(Semaphore::new(capacity)),
        }
    }

    pub(super) fn reserve_slot(&self) -> AoneResult<OwnedSemaphorePermit> {
        self.slots.clone().try_acquire_owned().map_err(|_| {
            AoneError::InvalidRequest(format!(
                "too many WebSocket sessions; maximum is {MAX_ACTIVE_SESSIONS}"
            ))
        })
    }

    pub(super) fn register(&self, session_id: String, control: SessionControl) -> AoneResult<()> {
        match self.sessions.entry(session_id) {
            Entry::Vacant(entry) => {
                entry.insert(control);
                Ok(())
            }
            Entry::Occupied(_) => Err(AoneError::Task(
                "generated WebSocket session id collision".into(),
            )),
        }
    }

    pub(super) fn outbound_sender(&self, session_id: &str) -> Option<mpsc::Sender<Message>> {
        self.sessions
            .get(session_id)
            .map(|control| control.outbound.clone())
    }

    pub(super) fn disconnect(&self, session_id: &str) -> bool {
        let Some((_, control)) = self.sessions.remove(session_id) else {
            return false;
        };
        let _ = control.shutdown.send(true);
        true
    }

    pub(super) fn finish(&self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    fn shutdown_all(&self) {
        let shutdown_senders = self
            .sessions
            .iter()
            .map(|control| control.shutdown.clone())
            .collect::<Vec<_>>();
        self.sessions.clear();
        for sender in shutdown_senders {
            let _ = sender.send(true);
        }
    }

    #[cfg(test)]
    pub(super) fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

pub struct WebSocketState {
    registry: Arc<SessionRegistry>,
}

impl WebSocketState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(SessionRegistry::with_capacity(MAX_ACTIVE_SESSIONS)),
        }
    }

    pub(super) fn registry(&self) -> Arc<SessionRegistry> {
        Arc::clone(&self.registry)
    }

    #[cfg(test)]
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            registry: Arc::new(SessionRegistry::with_capacity(capacity)),
        }
    }
}

impl Default for WebSocketState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WebSocketState {
    fn drop(&mut self) {
        self.registry.shutdown_all();
    }
}
