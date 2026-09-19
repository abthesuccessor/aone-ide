use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use uuid::Uuid;

use super::validation::content_hash;
use crate::{
    error::{AoneError, AoneResult},
    scanner::{canonicalize_relative_file, read_source_file},
};

#[derive(Debug)]
pub(super) struct WriteOutcome {
    pub(super) path: PathBuf,
    pub(super) metadata: Metadata,
    pub(super) durability_warning: Option<String>,
}

#[derive(Debug)]
struct FileIdentity {
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

struct TemporaryFile {
    path: PathBuf,
    armed: bool,
}

pub(super) fn atomic_write_if_unchanged(
    root: &Path,
    relative_path: &str,
    expected_content_hash: &str,
    content: &str,
) -> AoneResult<WriteOutcome> {
    let relative = Path::new(relative_path);
    let path = canonicalize_relative_file(root, relative)?;
    reject_symlink_components(root, relative)?;
    let before = path.symlink_metadata()?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(AoneError::SensitivePath);
    }
    let identity = FileIdentity::capture(&before);
    enforce_expected_hash(&path, expected_content_hash)?;

    let parent = path
        .parent()
        .ok_or_else(|| AoneError::InvalidRequest("workspace file has no parent".into()))?;
    // Open the directory before the commit point. A failure here is safe to
    // return because the original file has not been replaced yet.
    let parent_directory = File::open(parent)?;
    let temporary_path = parent.join(format!(".aone-save-{}.tmp", Uuid::now_v7()));
    let mut temporary = TemporaryFile {
        path: temporary_path.clone(),
        armed: true,
    };
    let mut file = open_temporary(&temporary_path)?;
    file.write_all(content.as_bytes())?;
    file.flush()?;
    file.set_permissions(before.permissions())?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    drop(file);

    revalidate_original(&path, &identity, expected_content_hash)?;
    fs::rename(&temporary_path, &path)?;
    temporary.armed = false;
    // Rename is the save commit point. Directory fsync improves crash
    // durability, but an fsync error must not make the renderer believe that
    // the now-visible new file was not saved.
    let durability_warning = parent_directory
        .sync_all()
        .err()
        .map(|error| error.to_string());
    Ok(WriteOutcome {
        path,
        metadata,
        durability_warning,
    })
}

fn open_temporary(path: &Path) -> AoneResult<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options.open(path).map_err(Into::into)
}

fn enforce_expected_hash(path: &Path, expected: &str) -> AoneResult<()> {
    let current = read_source_file(path)?;
    if content_hash(&current) != expected {
        return Err(AoneError::InvalidRequest(
            "file changed on disk; reload it before saving".into(),
        ));
    }
    Ok(())
}

fn revalidate_original(path: &Path, expected: &FileIdentity, hash: &str) -> AoneResult<()> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink()
        || path.canonicalize().ok().as_deref() != Some(path)
        || !expected.matches(&metadata)
    {
        return Err(AoneError::InvalidRequest(
            "file changed while the save was being prepared".into(),
        ));
    }
    enforce_expected_hash(path, hash)
}

fn reject_symlink_components(root: &Path, relative: &Path) -> AoneResult<()> {
    let mut cursor = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(AoneError::PathEscape);
        };
        cursor.push(component);
        let metadata = cursor.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            return Err(AoneError::SensitivePath);
        }
    }
    Ok(())
}

impl FileIdentity {
    fn capture(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt as _;
        Self {
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

    fn matches(&self, metadata: &Metadata) -> bool {
        self.length == metadata.len() && self.modified == metadata.modified().ok() && {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                self.device == metadata.dev()
                    && self.inode == metadata.ino()
                    && self.mode == metadata.mode()
                    && self.changed_seconds == metadata.ctime()
                    && self.changed_nanoseconds == metadata.ctime_nsec()
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}
