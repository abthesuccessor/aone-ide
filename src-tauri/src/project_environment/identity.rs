use std::{
    fs::Metadata,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::error::{AoneError, AoneResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExecutableIdentity {
    canonical_path: PathBuf,
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

pub(super) fn capture_executable_identity(path: &Path) -> AoneResult<ExecutableIdentity> {
    let canonical_path = path
        .canonicalize()
        .map_err(|_| AoneError::InvalidRequest("a detected executable no longer exists".into()))?;
    let metadata = canonical_path.metadata().map_err(|_| {
        AoneError::InvalidRequest("a detected executable cannot be inspected".into())
    })?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(AoneError::InvalidRequest(
            "a detected path is not a regular executable".into(),
        ));
    }
    if path.canonicalize().ok().as_deref() != Some(canonical_path.as_path()) {
        return Err(AoneError::InvalidRequest(
            "a detected executable changed while it was inspected".into(),
        ));
    }

    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;

    Ok(ExecutableIdentity {
        canonical_path,
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

pub(super) fn revalidate_executable_identity(
    launch_path: &Path,
    expected: &ExecutableIdentity,
) -> AoneResult<()> {
    let current = capture_executable_identity(launch_path)?;
    if &current != expected {
        return Err(AoneError::InvalidRequest(
            "a detected executable changed after approval; inspect again".into(),
        ));
    }
    Ok(())
}

pub(super) fn canonical_path(identity: &ExecutableIdentity) -> &Path {
    &identity.canonical_path
}

fn is_executable(metadata: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
