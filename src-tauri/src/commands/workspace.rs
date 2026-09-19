use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use super::graph::sync_graph_to_ai;
use crate::{
    ai::SecretState,
    analyzer::language_support,
    domain::{OpenFileResult, ScanProgress, SourceFile, WorkspaceFile, WorkspaceSummary},
    error::{AoneError, AoneResult},
    runner::RuntimeState,
    scanner::{
        IndexedFile, canonical_workspace, canonicalize_relative_file, read_source_file,
        relative_path, scan_workspace, workspace_id,
    },
    state::{AppState, WorkspaceAnalysisReservation, WorkspaceContext, new_workspace_context},
    store::GraphStore,
    terminal::TerminalState,
    watcher::start_workspace_watcher,
};

/// Opens a backend-owned native directory picker and installs only the folder
/// returned by that picker. The renderer never supplies a workspace path to a
/// privileged command.
#[tauri::command]
pub async fn pick_and_open_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
    terminal: State<'_, TerminalState>,
) -> AoneResult<Option<WorkspaceSummary>> {
    let _workspace_operation = state.try_reserve_workspace_operation()?;
    let selected = app
        .dialog()
        .file()
        .set_title("Open workspace in Aone")
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    terminal.begin_workspace_switch()?;
    let opened = open_workspace_at_path(path, app, &state, &runtime, &secrets).await;
    match opened {
        Ok(summary) => {
            terminal.complete_workspace_switch();
            Ok(Some(summary))
        }
        Err(error) => {
            terminal.cancel_workspace_switch();
            Err(error)
        }
    }
}

/// Opens a backend-owned native file picker, then installs and indexes the
/// selected file's containing directory. The renderer receives only the
/// resulting workspace and an index-relative path; it never supplies a path
/// to this privileged command.
#[tauri::command]
pub async fn pick_and_open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
    terminal: State<'_, TerminalState>,
) -> AoneResult<Option<OpenFileResult>> {
    let _workspace_operation = state.try_reserve_workspace_operation()?;
    let selected = app
        .dialog()
        .file()
        .set_title("Open file in Aone")
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    let (root, relative_path) = selected_file_target(&path)?;
    terminal.begin_workspace_switch()?;
    let opened = open_workspace_at_path(root, app, &state, &runtime, &secrets).await;
    match opened {
        Ok(workspace) => {
            terminal.complete_workspace_switch();
            Ok(Some(OpenFileResult {
                workspace,
                relative_path,
            }))
        }
        Err(error) => {
            terminal.cancel_workspace_switch();
            Err(error)
        }
    }
}

pub(super) fn selected_file_target(path: &Path) -> AoneResult<(PathBuf, String)> {
    let parent = path.parent().ok_or_else(|| {
        AoneError::InvalidRequest("selected file has no containing directory".into())
    })?;
    let root = canonical_workspace(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| AoneError::InvalidRequest("selected path is not a file".into()))?;
    let canonical_file = canonicalize_relative_file(&root, Path::new(file_name))?;
    let relative_path = relative_path(&root, &canonical_file)?;
    Ok((root, relative_path))
}

/// Internal path-taking helper kept outside Tauri's command registry. Native
/// pickers and backend-attested onboarding destinations can reuse it without
/// granting the renderer path-taking authority.
pub(crate) async fn open_workspace_at_path(
    path: PathBuf,
    app: AppHandle,
    state: &AppState,
    runtime: &RuntimeState,
    secrets: &SecretState,
) -> AoneResult<WorkspaceSummary> {
    let root = canonical_workspace(&path)?;
    runtime.begin_workspace_switch()?;
    let previous_context = state.workspace_if_open();
    let previous_analysis = match previous_context.clone() {
        Some(context) => match wait_for_context_analysis(context).await {
            Ok(reservation) => Some(reservation),
            Err(error) => {
                runtime.cancel_workspace_switch();
                return Err(error);
            }
        },
        None => None,
    };

    if let Some(context) = current_context_for_root(previous_context, &root) {
        let Some(reservation) = previous_analysis else {
            runtime.cancel_workspace_switch();
            return Err(AoneError::Task(
                "active workspace analysis guard was not acquired".into(),
            ));
        };
        let rescanned = rescan_context(app, state, secrets, context, reservation).await;
        return match rescanned {
            Ok(summary) => {
                runtime.complete_workspace_switch(root);
                Ok(summary)
            }
            Err(error) => {
                runtime.cancel_workspace_switch();
                Err(error)
            }
        };
    }

    let id = workspace_id(&root);
    let database_path = state.database_path(&id);
    let scan_root = root.clone();
    let scan_id = id.clone();
    let scan_app = app.clone();
    let prepared = async {
        let context = tauri::async_runtime::spawn_blocking(move || {
            let result = scan_workspace(&scan_root, &scan_id, |progress| {
                let _ = scan_app.emit("aone-scan-progress", progress);
            })?;
            let mut store = GraphStore::open(&database_path)?;
            commit_scan_result(&mut store, &result.files, |progress| {
                let _ = scan_app.emit("aone-scan-progress", progress);
            })?;
            new_workspace_context(scan_id, scan_root, store)
        })
        .await
        .map_err(|error| AoneError::Task(error.to_string()))??;
        let watcher = start_workspace_watcher(app.clone(), context.clone())?;
        Ok::<_, AoneError>((context, watcher))
    }
    .await;
    let (context, watcher) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            runtime.cancel_workspace_switch();
            return Err(error);
        }
    };

    // Let queued work on the old watcher finish before replacing it. The new
    // watcher is already observing the selected root, so there is no gap.
    drop(previous_analysis);
    state.install_workspace(context.clone(), watcher);
    let summary = refresh_committed_workspace(state, secrets, &context);
    runtime.complete_workspace_switch(root);
    emit_scan_complete(&app, &summary);
    Ok(summary)
}

#[tauri::command]
pub async fn rescan_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<WorkspaceSummary> {
    let _workspace_operation = state.try_reserve_workspace_operation()?;
    let context = state.workspace()?;
    let reservation = wait_for_context_analysis(context.clone()).await?;
    rescan_context(app, &state, &secrets, context, reservation).await
}

async fn rescan_context(
    app: AppHandle,
    state: &AppState,
    secrets: &SecretState,
    context: WorkspaceContext,
    _reservation: WorkspaceAnalysisReservation,
) -> AoneResult<WorkspaceSummary> {
    let scan_context = context.clone();
    let scan_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = scan_workspace(&scan_context.root, &scan_context.id, |progress| {
            let _ = scan_app.emit("aone-scan-progress", progress);
        })?;
        commit_scan_result(&mut scan_context.store.lock(), &result.files, |progress| {
            let _ = scan_app.emit("aone-scan-progress", progress);
        })
    })
    .await
    .map_err(|error| AoneError::Task(error.to_string()))??;
    let summary = refresh_committed_workspace(state, secrets, &context);
    emit_scan_complete(&app, &summary);
    Ok(summary)
}

pub(super) fn commit_scan_result<F>(
    store: &mut GraphStore,
    files: &[IndexedFile],
    mut progress: F,
) -> AoneResult<()>
where
    F: FnMut(ScanProgress),
{
    // A zero total explicitly means that SQLite graph/search persistence is
    // ongoing but cannot report honest per-row completion. The renderer keeps
    // its compact line indeterminate until the final `complete` event.
    progress(ScanProgress {
        phase: "committing".into(),
        completed: 0,
        total: 0,
        current_path: None,
    });
    store.replace_all(files)
}

pub(super) async fn wait_for_context_analysis(
    context: WorkspaceContext,
) -> AoneResult<WorkspaceAnalysisReservation> {
    tauri::async_runtime::spawn_blocking(move || context.wait_for_analysis())
        .await
        .map_err(|error| AoneError::Task(error.to_string()))
}

pub(super) fn current_context_for_root(
    context: Option<WorkspaceContext>,
    canonical_root: &Path,
) -> Option<WorkspaceContext> {
    context.filter(|candidate| candidate.root == canonical_root)
}

fn refresh_committed_workspace(
    state: &AppState,
    secrets: &SecretState,
    context: &WorkspaceContext,
) -> WorkspaceSummary {
    let summary = state.refresh_summary(&context.id).unwrap_or_else(|error| {
        eprintln!(
            "Aone IDE committed workspace {:?}, but summary refresh failed: {error}",
            context.root
        );
        context.summary.read().clone()
    });
    if let Err(error) = sync_graph_to_ai(state, context, secrets) {
        eprintln!(
            "Aone IDE committed workspace {:?}, but AI graph refresh failed: {error}",
            context.root
        );
        secrets.replace_graph_nodes(&context.id, Vec::new());
    }
    summary
}

fn emit_scan_complete(app: &AppHandle, summary: &WorkspaceSummary) {
    let _ = app.emit(
        "aone-scan-progress",
        ScanProgress {
            phase: "complete".into(),
            completed: summary.file_count,
            total: summary.file_count,
            current_path: None,
        },
    );
}

#[tauri::command]
pub fn get_workspace_files(
    query: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> AoneResult<Vec<WorkspaceFile>> {
    let context = state.workspace()?;
    context
        .store
        .lock()
        .list_files(query.as_deref(), limit.unwrap_or(500))
}

#[tauri::command]
pub fn read_workspace_file(
    relative_path: String,
    state: State<'_, AppState>,
) -> AoneResult<SourceFile> {
    let context = state.workspace()?;
    let indexed = context
        .store
        .lock()
        .get_file(&relative_path)?
        .ok_or_else(|| AoneError::InvalidRequest("file is not in the workspace index".into()))?;
    let path = canonicalize_relative_file(&context.root, PathBuf::from(&relative_path).as_path())?;
    let content = read_source_file(&path)?;
    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    Ok(SourceFile {
        relative_path,
        language: language_support(&path).name.into(),
        content,
        content_hash: if content_hash == indexed.content_hash {
            indexed.content_hash
        } else {
            content_hash
        },
    })
}
