use std::{
    env,
    fs::Metadata,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use super::{environment::validate_args, state::RuntimeState};
use crate::{
    domain::StartRunRequest,
    error::{AoneError, AoneResult},
};

#[derive(Debug)]
pub(super) struct PreparedRun {
    pub(super) executable: PathBuf,
    pub(super) executable_launch_path: PathBuf,
    pub(super) executable_identity: PathIdentity,
    pub(super) display_executable: String,
    pub(super) args: Vec<String>,
    pub(super) cwd: PathBuf,
    pub(super) cwd_identity: PathIdentity,
    pub(super) source: PathBuf,
    pub(super) source_identity: PathIdentity,
    pub(super) profile_id: String,
    pub(super) required_env: Vec<String>,
}

#[derive(Debug)]
pub(super) struct ResolvedExecutable {
    pub(super) launch_path: PathBuf,
    pub(super) target: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PathIdentity {
    canonical_path: PathBuf,
    kind: PathKind,
    length: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

pub(super) fn prepare_run(
    request: &StartRunRequest,
    state: &RuntimeState,
    workspace_root: &Path,
) -> AoneResult<PreparedRun> {
    if request.profile_id.trim().is_empty() {
        return Err(AoneError::InvalidRequest("profileId is required".into()));
    }

    let profile = state
        .registered_profile(&request.profile_id)
        .ok_or_else(|| {
            AoneError::InvalidRequest("run profile is not registered by the backend".into())
        })?;
    let raw_executable = profile.executable;
    let args = profile.args;
    let cwd_relative = profile.cwd_relative;
    let required_env = profile.required_env;
    let source = resolve_profile_source(workspace_root, &profile.source)?;

    validate_args(&args)?;
    let cwd = resolve_cwd(workspace_root, cwd_relative.as_deref())?;
    let resolved_executable = resolve_executable(workspace_root, &cwd, &raw_executable)?;
    let executable_identity = capture_path_identity(
        &resolved_executable.launch_path,
        PathKind::File,
        "executable",
    )?;
    if executable_identity.canonical_path != resolved_executable.target {
        return Err(AoneError::InvalidRequest(
            "executable target changed while preparing the run".into(),
        ));
    }
    let cwd_identity = capture_path_identity(&cwd, PathKind::Directory, "working directory")?;
    let source_identity = capture_path_identity(&source, PathKind::File, "profile source")?;

    Ok(PreparedRun {
        executable: resolved_executable.target,
        executable_launch_path: resolved_executable.launch_path,
        executable_identity,
        display_executable: raw_executable,
        args,
        cwd,
        cwd_identity,
        source,
        source_identity,
        profile_id: request.profile_id.clone(),
        required_env,
    })
}

impl PathIdentity {
    fn matches_revalidation(&self, current: &Self) -> bool {
        if self.canonical_path != current.canonical_path || self.kind != current.kind {
            return false;
        }

        #[cfg(unix)]
        if self.device != current.device || self.inode != current.inode || self.mode != current.mode
        {
            return false;
        }

        // A directory's size and timestamps legitimately change when entries in
        // the selected workspace change. Its canonical path and inode identify
        // the approved working directory. Files additionally bind size and both
        // modification/change timestamps to the consent decision.
        self.kind == PathKind::Directory
            || (self.length == current.length && self.modified == current.modified && {
                #[cfg(unix)]
                {
                    self.changed_seconds == current.changed_seconds
                        && self.changed_nanoseconds == current.changed_nanoseconds
                }
                #[cfg(not(unix))]
                {
                    true
                }
            })
    }
}

pub(super) fn capture_path_identity(
    path: &Path,
    expected_kind: PathKind,
    label: &str,
) -> AoneResult<PathIdentity> {
    let canonical_path = path.canonicalize().map_err(|_| {
        AoneError::InvalidRequest(format!("local process {label} no longer exists"))
    })?;
    let metadata = canonical_path.metadata().map_err(|_| {
        AoneError::InvalidRequest(format!("local process {label} cannot be inspected"))
    })?;
    let actual_kind = metadata_path_kind(&metadata).ok_or_else(|| {
        AoneError::InvalidRequest(format!(
            "local process {label} is not a regular file or directory"
        ))
    })?;
    if actual_kind != expected_kind {
        return Err(AoneError::InvalidRequest(format!(
            "local process {label} has an unexpected filesystem type"
        )));
    }

    if path.canonicalize().ok().as_deref() != Some(canonical_path.as_path()) {
        return Err(AoneError::InvalidRequest(format!(
            "local process {label} changed while it was inspected"
        )));
    }

    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;

    Ok(PathIdentity {
        canonical_path,
        kind: actual_kind,
        length: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(unix)]
        mode: metadata.mode(),
        #[cfg(unix)]
        changed_seconds: metadata.ctime(),
        #[cfg(unix)]
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

fn metadata_path_kind(metadata: &Metadata) -> Option<PathKind> {
    if metadata.is_file() {
        Some(PathKind::File)
    } else if metadata.is_dir() {
        Some(PathKind::Directory)
    } else {
        None
    }
}

pub(super) fn revalidate_path_identity(
    path: &Path,
    expected: &PathIdentity,
    label: &str,
) -> AoneResult<()> {
    let current = capture_path_identity(path, expected.kind, label)?;
    if !expected.matches_revalidation(&current) {
        return Err(AoneError::InvalidRequest(format!(
            "local process {label} changed after validation; review and try again"
        )));
    }
    Ok(())
}

pub(super) fn revalidate_prepared_run(prepared: &PreparedRun) -> AoneResult<()> {
    revalidate_path_identity(
        &prepared.executable_launch_path,
        &prepared.executable_identity,
        "executable",
    )?;
    revalidate_path_identity(&prepared.cwd, &prepared.cwd_identity, "working directory")?;
    revalidate_path_identity(
        &prepared.source,
        &prepared.source_identity,
        "profile source",
    )
}

fn resolve_profile_source(workspace_root: &Path, raw: &str) -> AoneResult<PathBuf> {
    let relative = Path::new(raw);
    if raw.trim().is_empty() || relative.is_absolute() {
        return Err(AoneError::InvalidRequest(
            "run profile source must be a workspace-relative file".into(),
        ));
    }
    reject_parent_components(relative)?;
    let canonical_workspace_root = workspace_root.canonicalize()?;
    let canonical = canonical_workspace_root
        .join(relative)
        .canonicalize()
        .map_err(|_| AoneError::InvalidRequest("run profile source does not exist".into()))?;
    if !canonical.starts_with(&canonical_workspace_root) {
        return Err(AoneError::PathEscape);
    }
    if !canonical.is_file() {
        return Err(AoneError::InvalidRequest(
            "run profile source is not a regular file".into(),
        ));
    }
    Ok(canonical)
}

pub(super) fn resolve_executable(
    workspace_root: &Path,
    cwd: &Path,
    raw: &str,
) -> AoneResult<ResolvedExecutable> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 1_024 || raw.contains(['\0', '\n', '\r']) {
        return Err(AoneError::InvalidRequest("invalid executable".into()));
    }

    let path = Path::new(raw);
    let contains_separator = path.components().count() > 1;
    if !path.is_absolute() && !contains_separator {
        let Some(search_path) = env::var_os("PATH") else {
            return Err(AoneError::InvalidRequest(format!(
                "executable `{raw}` was not found because PATH is unavailable"
            )));
        };
        for directory in env::split_paths(&search_path).filter(|directory| directory.is_absolute())
        {
            let Ok(canonical_directory) = directory.canonicalize() else {
                continue;
            };
            let candidate = canonical_directory.join(path);
            if candidate.is_file() {
                let target = candidate.canonicalize().map_err(|_| {
                    AoneError::InvalidRequest(format!(
                        "executable `{raw}` could not be resolved to a stable target"
                    ))
                })?;
                if !target.is_file() {
                    continue;
                }
                return Ok(ResolvedExecutable {
                    launch_path: candidate,
                    target,
                });
            }
        }
        return Err(AoneError::InvalidRequest(format!(
            "executable `{raw}` was not found on PATH"
        )));
    }

    let canonical_workspace_root = workspace_root.canonicalize()?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        reject_parent_components(path)?;
        cwd.join(path)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|_| AoneError::InvalidRequest("executable does not exist".into()))?;
    if !path.is_absolute() && !canonical.starts_with(&canonical_workspace_root) {
        return Err(AoneError::PathEscape);
    }
    if !canonical.is_file() {
        return Err(AoneError::InvalidRequest(
            "executable is not a regular file".into(),
        ));
    }
    Ok(ResolvedExecutable {
        launch_path: candidate,
        target: canonical,
    })
}

pub(super) fn resolve_cwd(workspace_root: &Path, relative: Option<&str>) -> AoneResult<PathBuf> {
    let Some(relative) = relative else {
        return Ok(workspace_root.to_path_buf());
    };
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err(AoneError::PathEscape);
    }
    reject_parent_components(relative)?;
    let canonical = workspace_root.join(relative).canonicalize()?;
    if !canonical.starts_with(workspace_root) {
        return Err(AoneError::PathEscape);
    }
    if !canonical.is_dir() {
        return Err(AoneError::InvalidRequest(
            "run working directory is not a directory".into(),
        ));
    }
    Ok(canonical)
}

fn reject_parent_components(path: &Path) -> AoneResult<()> {
    if path.components().any(|component| {
        !matches!(
            component,
            Component::Normal(_) | Component::CurDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AoneError::PathEscape);
    }
    Ok(())
}

pub(super) fn display_relative_cwd(workspace_root: &Path, cwd: &Path) -> String {
    cwd.strip_prefix(workspace_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}
