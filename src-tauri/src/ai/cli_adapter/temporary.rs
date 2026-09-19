use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use uuid::Uuid;

use super::{CliAdapterError, CliKind, JSON_SCHEMA};

pub(super) struct TemporaryWorkspace {
    pub(super) path: PathBuf,
}

impl TemporaryWorkspace {
    pub(super) fn new(kind: CliKind, with_schema: bool) -> Result<Self, CliAdapterError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("aone-ai-cli-{}-{suffix}", Uuid::new_v4().simple()));
        fs::create_dir(&path).map_err(|_| CliAdapterError::TemporaryWorkspace(kind.label()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(|_| CliAdapterError::TemporaryWorkspace(kind.label()))?;
        }
        let temporary = Self { path };
        if with_schema {
            temporary.write_schema(kind)?;
        }
        Ok(temporary)
    }

    fn write_schema(&self, kind: CliKind) -> Result<(), CliAdapterError> {
        let path = self.path.join("answer.schema.json");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|_| CliAdapterError::TemporaryWorkspace(kind.label()))?;
        file.write_all(JSON_SCHEMA)
            .map_err(|_| CliAdapterError::TemporaryWorkspace(kind.label()))
    }
}

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
