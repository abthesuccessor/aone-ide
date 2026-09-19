use std::{
    collections::HashMap,
    fs::{Metadata, OpenOptions},
    io::Read,
    path::Path,
    time::SystemTime,
};

use zeroize::Zeroize;

use super::parser::parse_env;
use crate::error::{AoneError, AoneResult};

const MAX_ENV_FILE_BYTES: u64 = 1024 * 1024;

/// Loads a path returned directly by the native backend dialog. Unlike renderer-supplied
/// paths this may be outside the workspace because the user attested the exact file.
pub fn load_attested_env_values(requested_path: &Path) -> AoneResult<HashMap<String, String>> {
    let selected_metadata = requested_path.symlink_metadata()?;
    if selected_metadata.file_type().is_symlink() || !selected_metadata.is_file() {
        return Err(AoneError::InvalidRequest(
            "environment selection must be a regular non-symlink file".into(),
        ));
    }
    let selected_identity = FileIdentity::capture(&selected_metadata);
    let canonical = requested_path.canonicalize()?;
    load_validated_env_file(&canonical, &selected_identity)
}

fn load_validated_env_file(
    canonical: &Path,
    selected_identity: &FileIdentity,
) -> AoneResult<HashMap<String, String>> {
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AoneError::InvalidRequest("environment path must be UTF-8".into()))?;
    if name != ".env" && !name.starts_with(".env.") && name != "local-runtime.env" {
        return Err(AoneError::InvalidRequest(
            "only explicit .env, .env.*, or local-runtime.env files can be loaded".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    let mut file = options.open(canonical)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !selected_identity.matches(&metadata) {
        return Err(AoneError::InvalidRequest(
            "environment file changed after native selection".into(),
        ));
    }
    if metadata.len() > MAX_ENV_FILE_BYTES {
        return Err(AoneError::FileTooLarge);
    }

    let mut content = String::with_capacity(metadata.len() as usize);
    if let Err(error) = file
        .by_ref()
        .take(MAX_ENV_FILE_BYTES + 1)
        .read_to_string(&mut content)
    {
        content.zeroize();
        return Err(AoneError::Io(error));
    }
    if content.len() as u64 > MAX_ENV_FILE_BYTES {
        content.zeroize();
        return Err(AoneError::FileTooLarge);
    }
    if !selected_identity.matches(&file.metadata()?) {
        content.zeroize();
        return Err(AoneError::InvalidRequest(
            "environment file changed while it was being read".into(),
        ));
    }
    let result = parse_env(&content);
    content.zeroize();
    result
}

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
