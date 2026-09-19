use std::{
    fs::Metadata,
    path::{Path, PathBuf},
};

use directories::UserDirs;

use crate::error::{AoneError, AoneResult};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

pub(super) fn documents_directory() -> AoneResult<(PathBuf, FileIdentity)> {
    let user_dirs = UserDirs::new().ok_or_else(|| {
        AoneError::Task("the current user's home directory is unavailable".into())
    })?;
    let configured = user_dirs.document_dir().ok_or_else(|| {
        AoneError::Task("the current user's Documents directory is unavailable".into())
    })?;
    let metadata = std::fs::symlink_metadata(configured).map_err(|_| {
        AoneError::Task("the current user's Documents directory is unavailable".into())
    })?;
    require_real_directory(configured, &metadata, "Documents")?;
    let canonical = configured.canonicalize()?;
    let canonical_metadata = std::fs::symlink_metadata(&canonical)?;
    require_real_directory(&canonical, &canonical_metadata, "Documents")?;
    Ok((canonical, identity(&canonical_metadata)))
}

pub(super) fn direct_child(documents: &Path, name: &str) -> AoneResult<PathBuf> {
    let target = documents.join(name);
    if target.parent() != Some(documents) {
        return Err(AoneError::InvalidRequest(
            "project destination is not a direct Documents child".into(),
        ));
    }
    Ok(target)
}

pub(super) fn ensure_identity(path: &Path, expected: &FileIdentity) -> AoneResult<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    require_real_directory(path, &metadata, "approved directory")?;
    if identity(&metadata) != *expected {
        return Err(AoneError::InvalidRequest(
            "approved directory identity changed before the operation".into(),
        ));
    }
    Ok(())
}

pub(super) fn ensure_absent(target: &Path) -> AoneResult<()> {
    match std::fs::symlink_metadata(target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(AoneError::InvalidRequest(
            "a file or folder already exists at the requested Documents destination".into(),
        )),
    }
}

pub(super) fn create_attested_directory(target: &Path) -> AoneResult<FileIdentity> {
    std::fs::create_dir(target)?;
    let metadata = std::fs::symlink_metadata(target)?;
    require_real_directory(target, &metadata, "new project")?;
    Ok(identity(&metadata))
}

pub(super) fn ensure_created_directory(
    documents: &Path,
    target: &Path,
    identity: &FileIdentity,
) -> AoneResult<()> {
    if target.parent() != Some(documents) {
        return Err(AoneError::InvalidRequest(
            "created project escaped the Documents directory".into(),
        ));
    }
    ensure_identity(target, identity)
}

fn require_real_directory(path: &Path, metadata: &Metadata, label: &str) -> AoneResult<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AoneError::InvalidRequest(format!(
            "{label} path is not a regular directory: {path:?}"
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn identity(metadata: &Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn identity(_metadata: &Metadata) -> FileIdentity {
    FileIdentity {}
}
