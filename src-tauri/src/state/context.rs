use std::{path::PathBuf, sync::Arc};

use parking_lot::{Condvar, Mutex, RwLock};

#[cfg(test)]
use crate::error::{AoneError, AoneResult};

use crate::{domain::WorkspaceSummary, store::GraphStore};

#[derive(Clone)]
pub struct WorkspaceContext {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub store: Arc<Mutex<GraphStore>>,
    pub summary: Arc<RwLock<WorkspaceSummary>>,
    pub(super) analysis: Arc<AnalysisCoordinator>,
}

#[derive(Default)]
pub(super) struct AnalysisCoordinator {
    active: Mutex<bool>,
    available: Condvar,
}

pub struct WorkspaceAnalysisReservation {
    coordinator: Arc<AnalysisCoordinator>,
}

impl Drop for WorkspaceAnalysisReservation {
    fn drop(&mut self) {
        let mut active = self.coordinator.active.lock();
        *active = false;
        self.coordinator.available.notify_one();
    }
}

impl WorkspaceContext {
    #[cfg(test)]
    pub fn try_reserve_analysis(&self) -> AoneResult<WorkspaceAnalysisReservation> {
        let mut active = self.analysis.active.lock();
        if *active {
            return Err(AoneError::InvalidRequest(
                "workspace analysis is already running; try again after it completes".into(),
            ));
        }
        *active = true;
        drop(active);
        Ok(WorkspaceAnalysisReservation {
            coordinator: Arc::clone(&self.analysis),
        })
    }

    pub fn wait_for_analysis(&self) -> WorkspaceAnalysisReservation {
        let mut active = self.analysis.active.lock();
        while *active {
            self.analysis.available.wait(&mut active);
        }
        *active = true;
        drop(active);
        WorkspaceAnalysisReservation {
            coordinator: Arc::clone(&self.analysis),
        }
    }
}
