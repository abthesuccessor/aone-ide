use std::{
    collections::BTreeSet,
    env,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use crate::{
    analyzer::stable_id,
    domain::TerminalProfile,
    error::{AoneError, AoneResult},
};

const MAX_SHELL_PATH_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PathIdentity {
    canonical: PathBuf,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedTerminalProfile {
    pub(super) public: TerminalProfile,
    pub(super) shell: PathBuf,
    identity: PathIdentity,
}

impl ResolvedTerminalProfile {
    pub(super) fn revalidate(&self) -> AoneResult<()> {
        let current = executable_identity(&self.shell)?;
        if current != self.identity {
            return Err(AoneError::InvalidRequest(
                "terminal shell changed after confirmation".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn list_profiles() -> Vec<ResolvedTerminalProfile> {
    let mut candidates = Vec::new();
    if let Some(value) = env::var_os("SHELL") {
        candidates.push(PathBuf::from(value));
    }
    for path in known_shell_paths() {
        candidates.push(PathBuf::from(path));
    }

    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter_map(|path| resolve_profile(&path).ok())
        .filter(|profile| seen.insert(profile.shell.clone()))
        .collect()
}

pub(super) fn find_profile(profile_id: &str) -> AoneResult<ResolvedTerminalProfile> {
    validate_profile_id(profile_id)?;
    list_profiles()
        .into_iter()
        .find(|profile| profile.public.id == profile_id)
        .ok_or_else(|| {
            AoneError::InvalidRequest(
                "terminal profile is unavailable; refresh detected profiles".into(),
            )
        })
}

pub(super) fn capture_directory_identity(path: &Path) -> AoneResult<PathIdentity> {
    let canonical = path.canonicalize()?;
    let metadata = canonical.metadata()?;
    if !metadata.is_dir() {
        return Err(AoneError::InvalidRequest(
            "terminal working directory is no longer a directory".into(),
        ));
    }
    Ok(identity_from_metadata(canonical, &metadata))
}

pub(super) fn revalidate_directory(path: &Path, expected: &PathIdentity) -> AoneResult<PathBuf> {
    let current = capture_directory_identity(path)?;
    if &current != expected {
        return Err(AoneError::InvalidRequest(
            "terminal working directory changed after confirmation".into(),
        ));
    }
    Ok(current.canonical)
}

fn resolve_profile(path: &Path) -> AoneResult<ResolvedTerminalProfile> {
    if !path.is_absolute() || path.as_os_str().len() > MAX_SHELL_PATH_BYTES {
        return Err(AoneError::InvalidRequest(
            "terminal shell path is invalid".into(),
        ));
    }
    let identity = executable_identity(path)?;
    let shell = identity.canonical.clone();
    let shell_path = shell.to_string_lossy().into_owned();
    let label = shell
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("shell")
        .to_owned();
    Ok(ResolvedTerminalProfile {
        public: TerminalProfile {
            id: stable_id("terminalProfile", &[&shell_path]),
            label,
            shell_path,
        },
        shell,
        identity,
    })
}

fn executable_identity(path: &Path) -> AoneResult<PathIdentity> {
    let canonical = path.canonicalize()?;
    let metadata = canonical.metadata()?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(AoneError::InvalidRequest(
            "terminal shell is not an executable file".into(),
        ));
    }
    Ok(identity_from_metadata(canonical, &metadata))
}

fn identity_from_metadata(canonical: PathBuf, metadata: &std::fs::Metadata) -> PathIdentity {
    PathIdentity {
        canonical,
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

fn validate_profile_id(value: &str) -> AoneResult<()> {
    if value.len() > 96
        || !value.starts_with("terminalProfile:")
        || value.len() == "terminalProfile:".len()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | '-'))
    {
        return Err(AoneError::InvalidRequest(
            "invalid terminal profile id".into(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn known_shell_paths() -> &'static [&'static str] {
    &[
        "/bin/zsh",
        "/bin/bash",
        "/bin/sh",
        "/opt/homebrew/bin/fish",
        "/usr/local/bin/fish",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn known_shell_paths() -> &'static [&'static str] {
    &["/bin/bash", "/bin/zsh", "/bin/fish", "/bin/sh"]
}

#[cfg(windows)]
fn known_shell_paths() -> &'static [&'static str] {
    &[]
}

#[cfg(test)]
pub(super) fn resolve_profile_for_test(path: &Path) -> AoneResult<ResolvedTerminalProfile> {
    resolve_profile(path)
}
