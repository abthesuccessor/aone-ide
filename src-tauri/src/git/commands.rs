use std::{fs, path::Path};

use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use super::{
    confirmation::{
        commit_confirmation_message, confirm_git_mutation, init_confirmation_message,
        path_confirmation_message,
    },
    inspection::repository_identity,
    paths::{validate_path, validate_paths},
    process::{git_failure, run_git, run_git_inspection, run_git_isolated},
    status::{RepositoryLocation, load_status, repository_location},
};
use crate::{
    domain::{
        GitCommitRequest, GitDiffRequest, GitDiffResult, GitMutationResult, GitPathsRequest,
        GitStatusResult,
    },
    error::{AoneError, AoneResult},
    state::AppState,
};

const DIFF_OUTPUT_LIMIT: usize = 512 * 1024;
const COMPARISON_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const INDEX_IDENTITY_OUTPUT_LIMIT: usize = 512 * 1024;
const MUTATION_OUTPUT_LIMIT: usize = 64 * 1024;
static GIT_MUTATION_LOCK: Mutex<()> = Mutex::const_new(());

enum ComparisonSide {
    Text(String),
    Missing,
    Unavailable,
    Truncated,
}

impl ComparisonSide {
    fn into_content(self) -> Option<String> {
        match self {
            Self::Text(content) => Some(content),
            Self::Missing => Some(String::new()),
            Self::Unavailable | Self::Truncated => None,
        }
    }

    fn is_truncated(&self) -> bool {
        matches!(self, Self::Truncated)
    }
}

#[tauri::command]
pub async fn get_git_status(state: State<'_, AppState>) -> AoneResult<GitStatusResult> {
    let context = state.workspace()?;
    load_status(&context.root).await
}

#[tauri::command]
pub async fn get_git_diff(
    request: GitDiffRequest,
    state: State<'_, AppState>,
) -> AoneResult<GitDiffResult> {
    let context = state.workspace()?;
    require_exact_repository(&context.root).await?;
    let relative_path = validate_path(&context.root, &request.relative_path)?;
    let mut args = vec![
        "diff".into(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
        "--ignore-submodules=all".into(),
        "--no-renames".into(),
        "--patch".into(),
    ];
    if request.staged {
        args.push("--cached".into());
    }
    args.push("--".into());
    args.push(relative_path.clone());
    let output = run_git_inspection(&context.root, &args, DIFF_OUTPUT_LIMIT).await?;
    if !output.success {
        return Err(git_failure("diff", &output));
    }
    let (original_content, modified_content, comparison_truncated) =
        load_comparison(&context.root, &relative_path, request.staged).await?;
    Ok(GitDiffResult {
        relative_path,
        staged: request.staged,
        content: String::from_utf8_lossy(&output.stdout).into_owned(),
        truncated: output.stdout_truncated,
        original_content,
        modified_content,
        comparison_truncated,
    })
}

pub(super) async fn load_comparison(
    root: &Path,
    relative_path: &str,
    staged: bool,
) -> AoneResult<(Option<String>, Option<String>, bool)> {
    let original_spec = if staged {
        format!("HEAD:{relative_path}")
    } else {
        format!(":{relative_path}")
    };
    let original = load_git_blob(root, original_spec).await?;
    let modified = if staged {
        load_git_blob(root, format!(":{relative_path}")).await?
    } else {
        load_working_tree_text(root, relative_path)?
    };
    let comparison_truncated = original.is_truncated() || modified.is_truncated();
    Ok((
        original.into_content(),
        modified.into_content(),
        comparison_truncated,
    ))
}

async fn load_git_blob(root: &Path, object: String) -> AoneResult<ComparisonSide> {
    let output = run_git_inspection(
        root,
        &["show".into(), "--no-textconv".into(), object],
        COMPARISON_OUTPUT_LIMIT,
    )
    .await?;
    if !output.success {
        return Ok(ComparisonSide::Missing);
    }
    if output.stdout_truncated {
        return Ok(ComparisonSide::Truncated);
    }
    Ok(match String::from_utf8(output.stdout) {
        Ok(content) => ComparisonSide::Text(content),
        Err(_) => ComparisonSide::Unavailable,
    })
}

fn load_working_tree_text(root: &Path, relative_path: &str) -> AoneResult<ComparisonSide> {
    let candidate = root.join(relative_path);
    let metadata = match fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ComparisonSide::Missing);
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AoneError::SensitivePath);
    }
    if metadata.len() > COMPARISON_OUTPUT_LIMIT as u64 {
        return Ok(ComparisonSide::Truncated);
    }
    Ok(match String::from_utf8(fs::read(candidate)?) {
        Ok(content) => ComparisonSide::Text(content),
        Err(_) => ComparisonSide::Unavailable,
    })
}

#[tauri::command]
pub async fn git_stage(
    request: GitPathsRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<GitMutationResult> {
    mutate_paths(request, app, state, PathMutation::Stage).await
}

#[tauri::command]
pub async fn git_unstage(
    request: GitPathsRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<GitMutationResult> {
    mutate_paths(request, app, state, PathMutation::Unstage).await
}

#[tauri::command]
pub async fn git_commit(
    request: GitCommitRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<GitMutationResult> {
    let _mutation_guard = GIT_MUTATION_LOCK.lock().await;
    let context = state.workspace()?;
    require_exact_repository(&context.root).await?;
    let message = validate_commit_message(&request.message)?;
    let approved = staged_snapshot(&context.root).await?;
    confirm_git_mutation(
        &app,
        "Confirm local Git commit",
        commit_confirmation_message(&context.root, &message, &approved.paths),
        "Commit locally",
    )
    .await?;
    ensure_workspace_unchanged(&state, &context.id)?;
    require_exact_repository(&context.root).await?;
    if staged_snapshot(&context.root).await? != approved {
        return Err(AoneError::InvalidRequest(
            "staged paths or content changed while the commit was awaiting confirmation".into(),
        ));
    }
    let args = vec![
        "commit".into(),
        "--no-verify".into(),
        "--no-gpg-sign".into(),
        "-m".into(),
        message,
    ];
    let output = run_git(&context.root, &args, MUTATION_OUTPUT_LIMIT).await?;
    if !output.success {
        return Err(git_failure("commit", &output));
    }
    Ok(GitMutationResult {
        ok: true,
        summary: "Created a local commit. No remote action was performed.".into(),
    })
}

#[tauri::command]
pub async fn git_initialize(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<GitMutationResult> {
    let _mutation_guard = GIT_MUTATION_LOCK.lock().await;
    let context = state.workspace()?;
    match repository_location(&context.root).await? {
        RepositoryLocation::Exact => {
            return Ok(GitMutationResult {
                ok: true,
                summary: "This workspace is already a Git repository.".into(),
            });
        }
        RepositoryLocation::Ancestor => {
            return Err(AoneError::InvalidRequest(
                "the workspace is inside another repository; open its Git root instead".into(),
            ));
        }
        RepositoryLocation::None => {}
    }
    confirm_git_mutation(
        &app,
        "Initialize local Git repository",
        init_confirmation_message(&context.root),
        "Initialize Git",
    )
    .await?;
    ensure_workspace_unchanged(&state, &context.id)?;
    match repository_location(&context.root).await? {
        RepositoryLocation::Exact => {
            return Ok(GitMutationResult {
                ok: true,
                summary: "This workspace became a Git repository before initialization.".into(),
            });
        }
        RepositoryLocation::Ancestor => {
            return Err(AoneError::InvalidRequest(
                "the workspace became nested inside another repository".into(),
            ));
        }
        RepositoryLocation::None => {}
    }
    let output = run_git_isolated(
        &context.root,
        &["init".into(), "--template=".into()],
        MUTATION_OUTPUT_LIMIT,
    )
    .await?;
    if !output.success {
        return Err(git_failure("initialization", &output));
    }
    Ok(GitMutationResult {
        ok: true,
        summary: "Initialized a local Git repository without a remote.".into(),
    })
}

#[derive(Debug, Clone, Copy)]
enum PathMutation {
    Stage,
    Unstage,
}

async fn mutate_paths(
    request: GitPathsRequest,
    app: AppHandle,
    state: State<'_, AppState>,
    mutation: PathMutation,
) -> AoneResult<GitMutationResult> {
    let _mutation_guard = GIT_MUTATION_LOCK.lock().await;
    let context = state.workspace()?;
    require_exact_repository(&context.root).await?;
    let paths = validate_paths(&context.root, &request.paths)?;
    let (operation, title, button, filters) = match mutation {
        PathMutation::Stage => ("stage", "Confirm local Git staging", "Stage paths", true),
        PathMutation::Unstage => (
            "unstage",
            "Confirm local Git unstaging",
            "Unstage paths",
            false,
        ),
    };
    confirm_git_mutation(
        &app,
        title,
        path_confirmation_message(operation, &context.root, &paths, filters),
        button,
    )
    .await?;
    ensure_workspace_unchanged(&state, &context.id)?;
    require_exact_repository(&context.root).await?;
    let paths = validate_paths(&context.root, &paths)?;
    let mut args = match mutation {
        PathMutation::Stage => vec!["add".into(), "-A".into(), "--".into()],
        PathMutation::Unstage => unstage_args(repository_has_head(&context.root).await?),
    };
    args.extend(paths.iter().cloned());
    let output = run_git(&context.root, &args, MUTATION_OUTPUT_LIMIT).await?;
    if !output.success {
        return Err(git_failure(operation, &output));
    }
    Ok(GitMutationResult {
        ok: true,
        summary: format!("Git {operation} completed for {} path(s).", paths.len()),
    })
}

pub(super) fn unstage_args(has_head: bool) -> Vec<String> {
    if has_head {
        return vec!["restore".into(), "--staged".into(), "--".into()];
    }
    // An unborn repository can have an AM path whose index differs from the
    // working tree. Force removes only the validated index entry; it does not
    // delete or overwrite the working-tree file.
    vec![
        "rm".into(),
        "--cached".into(),
        "-f".into(),
        "--ignore-unmatch".into(),
        "--".into(),
    ]
}

async fn repository_has_head(root: &std::path::Path) -> AoneResult<bool> {
    Ok(repository_identity(root)?.1.is_some())
}

pub(super) async fn staged_paths(root: &std::path::Path) -> AoneResult<Vec<String>> {
    let output = run_git_inspection(
        root,
        &[
            "diff".into(),
            "--cached".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--ignore-submodules=all".into(),
            "--name-only".into(),
            "-z".into(),
            "--no-renames".into(),
            "--".into(),
        ],
        256 * 1024,
    )
    .await?;
    if !output.success {
        return Err(git_failure("staged-path inspection", &output));
    }
    if output.stdout_truncated {
        return Err(AoneError::InvalidRequest(
            "staged path list exceeds the supported limit".into(),
        ));
    }
    let raw_paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            std::str::from_utf8(field).map(str::to_owned).map_err(|_| {
                AoneError::InvalidRequest("non-UTF-8 Git paths are not supported".into())
            })
        })
        .collect::<AoneResult<Vec<_>>>()?;
    if raw_paths.is_empty() {
        return Err(AoneError::InvalidRequest(
            "there are no staged paths to commit".into(),
        ));
    }
    validate_paths(root, &raw_paths)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct StagedSnapshot {
    pub(super) paths: Vec<String>,
    pub(super) identity: [u8; 32],
}

pub(super) async fn staged_snapshot(root: &std::path::Path) -> AoneResult<StagedSnapshot> {
    // Capture the raw-index identity on both sides of path enumeration. This
    // fails closed if another local Git client is changing the index while the
    // native confirmation snapshot is being prepared.
    for _ in 0..3 {
        let before = staged_index_identity(root).await?;
        let paths = staged_paths(root).await?;
        let after = staged_index_identity(root).await?;
        if before == after {
            return Ok(StagedSnapshot {
                paths,
                identity: after,
            });
        }
    }
    Err(AoneError::InvalidRequest(
        "Git index is changing; wait for other Git actions before committing".into(),
    ))
}

async fn staged_index_identity(root: &std::path::Path) -> AoneResult<[u8; 32]> {
    let output = run_git_inspection(
        root,
        &[
            "diff".into(),
            "--cached".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--ignore-submodules=all".into(),
            "--raw".into(),
            "-z".into(),
            "--full-index".into(),
            "--no-renames".into(),
            "--".into(),
        ],
        INDEX_IDENTITY_OUTPUT_LIMIT,
    )
    .await?;
    if !output.success {
        return Err(git_failure("staged-content inspection", &output));
    }
    if output.stdout_truncated {
        return Err(AoneError::InvalidRequest(
            "staged content identity exceeds the supported limit".into(),
        ));
    }
    Ok(*blake3::hash(&output.stdout).as_bytes())
}

async fn require_exact_repository(root: &std::path::Path) -> AoneResult<()> {
    match repository_location(root).await? {
        RepositoryLocation::Exact => Ok(()),
        RepositoryLocation::Ancestor => Err(AoneError::InvalidRequest(
            "open the repository root before using Git actions".into(),
        )),
        RepositoryLocation::None => Err(AoneError::InvalidRequest(
            "the open workspace is not a Git repository".into(),
        )),
    }
}

fn validate_commit_message(raw: &str) -> AoneResult<String> {
    let message = raw.trim();
    if message.is_empty() || message.len() > 4 * 1024 || message.contains('\0') {
        return Err(AoneError::InvalidRequest(
            "commit message must contain between 1 and 4096 bytes".into(),
        ));
    }
    if message.chars().any(|value| matches!(value, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) {
        return Err(AoneError::InvalidRequest(
            "commit message contains direction-override characters".into(),
        ));
    }
    Ok(message.to_owned())
}

fn ensure_workspace_unchanged(state: &AppState, expected_id: &str) -> AoneResult<()> {
    if state.workspace()?.id != expected_id {
        return Err(AoneError::InvalidRequest(
            "workspace changed while the Git action was awaiting confirmation".into(),
        ));
    }
    Ok(())
}
