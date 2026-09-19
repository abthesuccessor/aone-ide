use std::{
    fs::{self, Metadata, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use uuid::Uuid;

use crate::error::{AoneError, AoneResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfigTargetAttestation {
    approved_path: PathBuf,
    parent_identities: Vec<PathIdentity>,
    target_identity: Option<PathIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathIdentity {
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

impl ConfigTargetAttestation {
    pub(super) fn approved_path(&self) -> &Path {
        &self.approved_path
    }

    pub(super) fn exists(&self) -> bool {
        self.target_identity.is_some()
    }

    pub(super) fn existing_canonical_path(&self) -> AoneResult<&Path> {
        self.target_identity
            .as_ref()
            .map(|identity| identity.canonical_path.as_path())
            .ok_or_else(|| AoneError::Task("tool configuration does not exist".into()))
    }
}

pub(super) fn attest_target(
    trusted_root: &Path,
    target: &Path,
) -> AoneResult<ConfigTargetAttestation> {
    validate_target_shape(trusted_root, target)?;
    let mut parent_identities = vec![capture_directory_identity(trusted_root, trusted_root)?];
    let relative_parent = target
        .parent()
        .and_then(|parent| parent.strip_prefix(trusted_root).ok())
        .ok_or(AoneError::PathEscape)?;
    let mut current = PathBuf::from(trusted_root);
    let mut missing_parent = false;
    for component in relative_parent.components() {
        let Component::Normal(value) = component else {
            return Err(AoneError::PathEscape);
        };
        current.push(value);
        match current.symlink_metadata() {
            Ok(_) if missing_parent => {
                return Err(AoneError::InvalidRequest(
                    "configuration path changed during inspection".into(),
                ));
            }
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AoneError::SensitivePath);
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(AoneError::InvalidRequest(
                    "configuration parent is not a directory".into(),
                ));
            }
            Ok(_) => parent_identities.push(capture_directory_identity(&current, trusted_root)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_parent = true;
            }
            Err(error) => return Err(AoneError::Io(error)),
        }
    }

    let target_identity = match target.symlink_metadata() {
        Ok(_) if missing_parent => {
            return Err(AoneError::InvalidRequest(
                "configuration path changed during inspection".into(),
            ));
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(AoneError::SensitivePath);
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(AoneError::InvalidRequest(
                "tool configuration path is not a regular file".into(),
            ));
        }
        Ok(metadata) => Some(capture_file_identity(target, metadata)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(AoneError::Io(error)),
    };
    let approved_path = target_identity
        .as_ref()
        .map(|identity| identity.canonical_path.clone())
        .unwrap_or_else(|| target.to_path_buf());
    if !approved_path.starts_with(trusted_root) {
        return Err(AoneError::PathEscape);
    }
    Ok(ConfigTargetAttestation {
        approved_path,
        parent_identities,
        target_identity,
    })
}

pub(super) fn revalidate_target(
    trusted_root: &Path,
    target: &Path,
    expected: &ConfigTargetAttestation,
) -> AoneResult<ConfigTargetAttestation> {
    let current = attest_target(trusted_root, target)?;
    if &current != expected {
        return Err(AoneError::InvalidRequest(
            "tool configuration identity or containment changed after confirmation".into(),
        ));
    }
    Ok(current)
}

pub(super) fn create_config_atomically(
    trusted_root: &Path,
    target: &Path,
    template: &str,
) -> AoneResult<bool> {
    if attest_target(trusted_root, target)?.exists() {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| AoneError::InvalidRequest("configuration has no parent directory".into()))?;
    create_safe_directories(trusted_root, parent)?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(trusted_root) {
        return Err(AoneError::PathEscape);
    }
    if attest_target(trusted_root, target)?.exists() {
        return Ok(false);
    }

    let temp = temporary_path(&canonical_parent);
    let created =
        write_private_temp(&temp, template.as_bytes()).and_then(|()| {
            match fs::hard_link(&temp, target) {
                Ok(()) => Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    attest_target(trusted_root, target).map(|_| false)
                }
                Err(error) => Err(AoneError::Io(error)),
            }
        });
    let _ = fs::remove_file(&temp);
    created
}

fn validate_target_shape(trusted_root: &Path, target: &Path) -> AoneResult<()> {
    let canonical_root = trusted_root.canonicalize()?;
    if canonical_root != trusted_root || !canonical_root.is_dir() {
        return Err(AoneError::PathEscape);
    }
    let relative = target
        .strip_prefix(trusted_root)
        .map_err(|_| AoneError::PathEscape)?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AoneError::PathEscape);
    }
    Ok(())
}

fn capture_directory_identity(path: &Path, trusted_root: &Path) -> AoneResult<PathIdentity> {
    let lexical = path.symlink_metadata()?;
    if lexical.file_type().is_symlink() || !lexical.is_dir() {
        return Err(AoneError::SensitivePath);
    }
    let canonical = path.canonicalize()?;
    let canonical_metadata = canonical.metadata()?;
    if !canonical.starts_with(trusted_root)
        || !canonical_metadata.is_dir()
        || !same_filesystem_identity(&lexical, &canonical_metadata)
    {
        return Err(AoneError::PathEscape);
    }
    Ok(capture_identity(canonical, canonical_metadata))
}

fn capture_file_identity(path: &Path, lexical: Metadata) -> AoneResult<PathIdentity> {
    let canonical = path.canonicalize()?;
    let canonical_metadata = canonical.metadata()?;
    if !canonical_metadata.is_file() || !same_filesystem_identity(&lexical, &canonical_metadata) {
        return Err(AoneError::InvalidRequest(
            "configuration changed during inspection".into(),
        ));
    }
    Ok(capture_identity(canonical, canonical_metadata))
}

fn capture_identity(canonical_path: PathBuf, metadata: Metadata) -> PathIdentity {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;

    PathIdentity {
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
    }
}

#[cfg(unix)]
fn same_filesystem_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino() && left.mode() == right.mode()
}

#[cfg(not(unix))]
fn same_filesystem_identity(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn create_safe_directories(trusted_root: &Path, parent: &Path) -> AoneResult<()> {
    let relative = parent
        .strip_prefix(trusted_root)
        .map_err(|_| AoneError::PathEscape)?;
    let mut current = PathBuf::from(trusted_root);
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(AoneError::PathEscape);
        };
        current.push(value);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AoneError::SensitivePath);
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(AoneError::InvalidRequest(
                    "configuration parent is not a directory".into(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_private_directory(&current)?;
            }
            Err(error) => return Err(AoneError::Io(error)),
        }
        if !current.canonicalize()?.starts_with(trusted_root) {
            return Err(AoneError::PathEscape);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> AoneResult<()> {
    use std::os::unix::fs::DirBuilderExt as _;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(AoneError::Io(error)),
    }
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> AoneResult<()> {
    fs::create_dir(path).map_err(AoneError::Io)
}

fn write_private_temp(path: &Path, content: &[u8]) -> AoneResult<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    Ok(())
}

fn temporary_path(parent: &Path) -> PathBuf {
    parent.join(format!(".aone-config-{}.tmp", Uuid::now_v7()))
}
