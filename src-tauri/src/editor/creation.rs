use std::{
    fs::{File, OpenOptions},
    path::{Component, Path},
    time::Instant,
};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use super::{commands::complete_committed_save, validation::content_hash};
use crate::{
    analyzer::{analyze_source_with_deadline, language_support, stable_id},
    domain::{SourceFile, WorkspaceFile},
    error::{AoneError, AoneResult},
    scanner::{
        IndexedFile, MAX_WORKSPACE_SCAN_DURATION, is_hard_denied, relative_path, system_time_string,
    },
    state::AppState,
};

#[tauri::command]
pub async fn pick_and_create_workspace_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<Option<SourceFile>> {
    let _workspace_operation = state.try_reserve_workspace_operation()?;
    let context = state.workspace()?;
    let selected = app
        .dialog()
        .file()
        .set_title("Create file in Aone")
        .set_directory(&context.root)
        .set_file_name("untitled.txt")
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let selected = selected
        .into_path()
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    let (target, relative_path) = validate_create_target(&context.root, &selected)?;
    let workspace_id = context.id.clone();
    let commit_workspace_id = workspace_id.clone();
    let app_handle = app.clone();
    let (source, reindexed) = tauri::async_runtime::spawn_blocking(move || {
        let _analysis = context.wait_for_analysis();
        let hash = content_hash("");
        let deadline = Instant::now()
            .checked_add(MAX_WORKSPACE_SCAN_DURATION)
            .ok_or_else(|| AoneError::Task("source analysis deadline is unavailable".into()))?;
        let analysis =
            analyze_source_with_deadline(&context.id, &relative_path, "", &hash, Some(deadline))?;
        app_handle
            .state::<AppState>()
            .while_workspace_current(&commit_workspace_id, || {
                let metadata = create_empty_file(&target)?;
                let support = language_support(&target);
                let indexed = IndexedFile {
                    file: WorkspaceFile {
                        id: stable_id("file", &[&context.id, &relative_path]),
                        relative_path: relative_path.clone(),
                        language: support.name.into(),
                        capability: support.capability,
                        size_bytes: 0,
                        content_hash: hash.clone(),
                        modified_at: metadata
                            .modified()
                            .ok()
                            .map(system_time_string)
                            .unwrap_or_else(|| "unknown".into()),
                        parse_errors: analysis.parse_errors,
                    },
                    source: String::new(),
                    analysis,
                };
                let source = SourceFile {
                    relative_path,
                    language: support.name.into(),
                    content: String::new(),
                    content_hash: hash,
                };
                Ok(complete_committed_save(&context, &indexed, source))
            })
    })
    .await
    .map_err(|error| AoneError::Task(error.to_string()))??;

    if reindexed {
        let _ = state.refresh_summary(&workspace_id);
    }
    Ok(Some(source))
}

fn validate_create_target(
    root: &Path,
    selected: &Path,
) -> AoneResult<(std::path::PathBuf, String)> {
    let parent = selected
        .parent()
        .ok_or_else(|| AoneError::InvalidRequest("selected file has no parent folder".into()))?
        .canonicalize()?;
    if !parent.starts_with(root) {
        return Err(AoneError::PathEscape);
    }
    let name = selected
        .file_name()
        .ok_or_else(|| AoneError::InvalidRequest("enter a file name".into()))?;
    if selected.components().any(|component| {
        !matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::Normal(_)
        )
    }) {
        return Err(AoneError::PathEscape);
    }
    let target = parent.join(name);
    if target.symlink_metadata().is_ok() {
        return Err(AoneError::InvalidRequest(
            "a file already exists at the selected path".into(),
        ));
    }
    let relative = target
        .strip_prefix(root)
        .map_err(|_| AoneError::PathEscape)?;
    if is_hard_denied(relative) {
        return Err(AoneError::SensitivePath);
    }
    Ok((target.clone(), relative_path(root, &target)?))
}

fn create_empty_file(path: &Path) -> AoneResult<std::fs::Metadata> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    drop(file);
    if let Some(parent) = path.parent()
        && let Err(error) = File::open(parent).and_then(|directory| directory.sync_all())
    {
        eprintln!(
            "Aone IDE created {path:?}, but parent-directory fsync failed: {error}. The file is visible on disk."
        );
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::validate_create_target;
    use crate::error::AoneError;

    #[test]
    fn validates_new_targets_inside_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        std::fs::create_dir(root_path.join("src")).unwrap();
        let target = root_path.join("src/new.ts");
        let (_, relative) = validate_create_target(&root_path, &target).unwrap();
        assert_eq!(relative, "src/new.ts");
    }

    #[test]
    fn rejects_existing_and_sensitive_targets() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        std::fs::write(root_path.join("existing.ts"), "x").unwrap();
        assert!(matches!(
            validate_create_target(&root_path, &root_path.join("existing.ts")),
            Err(AoneError::InvalidRequest(_))
        ));
        assert!(matches!(
            validate_create_target(&root_path, &root_path.join(".env")),
            Err(AoneError::SensitivePath)
        ));
    }
}
