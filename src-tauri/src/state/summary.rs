use std::{path::PathBuf, sync::Arc};

use parking_lot::{Mutex, RwLock};

use super::context::{AnalysisCoordinator, WorkspaceContext};
use crate::{
    domain::{LanguageSummary, WorkspaceSummary},
    error::AoneResult,
    scanner::system_time_string,
    store::GraphStore,
};

pub fn new_workspace_context(
    id: String,
    root: PathBuf,
    store: GraphStore,
) -> AoneResult<WorkspaceContext> {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Workspace")
        .to_owned();
    let context = WorkspaceContext {
        id: id.clone(),
        name: name.clone(),
        root: root.clone(),
        store: Arc::new(Mutex::new(store)),
        summary: Arc::new(RwLock::new(WorkspaceSummary {
            id,
            name,
            root_path: root.to_string_lossy().into_owned(),
            file_count: 0,
            node_count: 0,
            edge_count: 0,
            languages: Vec::new(),
            last_scanned_at: system_time_string(std::time::SystemTime::now()),
        })),
        analysis: Arc::new(AnalysisCoordinator::default()),
    };
    let summary = build_summary(&context)?;
    *context.summary.write() = summary;
    Ok(context)
}

pub(super) fn build_summary(context: &WorkspaceContext) -> AoneResult<WorkspaceSummary> {
    let store = context.store.lock();
    let (file_count, node_count, edge_count) = store.counts()?;
    let languages = store
        .language_counts()?
        .into_iter()
        .map(|(language, capability, file_count)| LanguageSummary {
            language,
            file_count,
            capability,
        })
        .collect();
    Ok(WorkspaceSummary {
        id: context.id.clone(),
        name: context.name.clone(),
        root_path: context.root.to_string_lossy().into_owned(),
        file_count,
        node_count,
        edge_count,
        languages,
        last_scanned_at: system_time_string(std::time::SystemTime::now()),
    })
}
