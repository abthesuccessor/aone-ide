use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use parking_lot::{Mutex, RwLock};

use super::{WorkspaceContext, summary::build_summary};
use crate::{
    domain::WorkspaceSummary,
    error::{AoneError, AoneResult},
    watcher::WorkspaceWatcher,
};

pub struct AppState {
    app_data_root: PathBuf,
    current: RwLock<Option<WorkspaceContext>>,
    watcher: Mutex<Option<WorkspaceWatcher>>,
    workspace_operation: Arc<AtomicBool>,
}

pub(crate) struct WorkspaceOperationReservation {
    active: Arc<AtomicBool>,
}

impl Drop for WorkspaceOperationReservation {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

impl AppState {
    pub fn new(app_data_root: PathBuf) -> AoneResult<Self> {
        std::fs::create_dir_all(app_data_root.join("workspaces"))?;
        Ok(Self {
            app_data_root,
            current: RwLock::new(None),
            watcher: Mutex::new(None),
            workspace_operation: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Serializes renderer-requested full scans and workspace switches. File
    /// watcher work remains coordinated by the current workspace reservation.
    pub(crate) fn try_reserve_workspace_operation(
        &self,
    ) -> AoneResult<WorkspaceOperationReservation> {
        self.workspace_operation
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                AoneError::InvalidRequest(
                    "workspace indexing or folder switching is already in progress".into(),
                )
            })?;
        Ok(WorkspaceOperationReservation {
            active: Arc::clone(&self.workspace_operation),
        })
    }

    pub fn database_path(&self, workspace_id: &str) -> PathBuf {
        let safe_id = workspace_id.replace(':', "_");
        self.app_data_root
            .join("workspaces")
            .join(safe_id)
            .join("aone.sqlite")
    }

    pub fn workspace(&self) -> AoneResult<WorkspaceContext> {
        self.current
            .read()
            .clone()
            .ok_or(AoneError::WorkspaceNotOpen)
    }

    pub fn workspace_if_open(&self) -> Option<WorkspaceContext> {
        self.current.read().clone()
    }

    /// Holds the current-workspace read lock across one short commit section.
    /// Workspace installation must take the matching write lock, so an
    /// operation cannot commit against a newly installed workspace.
    pub fn while_workspace_current<T>(
        &self,
        expected_workspace_id: &str,
        operation: impl FnOnce() -> AoneResult<T>,
    ) -> AoneResult<T> {
        let current = self.current.read();
        let matches = current
            .as_ref()
            .is_some_and(|context| context.id == expected_workspace_id);
        if !matches {
            return Err(AoneError::InvalidRequest(
                "workspace changed while the operation was running".into(),
            ));
        }
        operation()
    }

    pub fn install_workspace(&self, context: WorkspaceContext, watcher: WorkspaceWatcher) {
        *self.watcher.lock() = None;
        *self.current.write() = Some(context);
        *self.watcher.lock() = Some(watcher);
    }

    #[cfg(test)]
    pub(crate) fn install_workspace_for_test(&self, context: WorkspaceContext) {
        *self.current.write() = Some(context);
    }

    pub fn refresh_summary(&self, expected_workspace_id: &str) -> AoneResult<WorkspaceSummary> {
        let context = self.workspace()?;
        if context.id != expected_workspace_id {
            return Err(AoneError::InvalidRequest(
                "workspace changed while analysis was running".into(),
            ));
        }
        let summary = build_summary(&context)?;
        *context.summary.write() = summary.clone();
        Ok(summary)
    }
}
