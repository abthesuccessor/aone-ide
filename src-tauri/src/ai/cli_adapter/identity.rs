use std::{
    fs::{self, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::{CliAdapter, CliAdapterError, CliKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExecutableIdentity {
    pub(super) canonical_path: PathBuf,
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

pub(super) fn capture_identity(
    kind: CliKind,
    launch_path: &Path,
    home: &Path,
    workspace: Option<&Path>,
) -> Result<ExecutableIdentity, CliAdapterError> {
    let canonical_path = launch_path
        .canonicalize()
        .map_err(|_| CliAdapterError::UnsafeExecutable(kind.label()))?;
    let metadata = canonical_path
        .metadata()
        .map_err(|_| CliAdapterError::UnsafeExecutable(kind.label()))?;
    if !metadata.is_file()
        || !is_executable(&metadata)
        || !is_native_executable(&canonical_path)
        || !canonical_target_allowed(kind, &canonical_path, home)
        || workspace
            .is_some_and(|root| launch_path.starts_with(root) || canonical_path.starts_with(root))
    {
        return Err(CliAdapterError::UnsafeExecutable(kind.label()));
    }
    Ok(identity_from(canonical_path, &metadata))
}

pub(super) fn revalidate_identity(adapter: &CliAdapter) -> Result<(), CliAdapterError> {
    #[cfg(test)]
    if adapter.test_executable {
        let current = capture_test_identity(&adapter.launch_path)?;
        return (current == adapter.identity)
            .then_some(())
            .ok_or(CliAdapterError::IdentityChanged(adapter.kind.label()));
    }
    let current = capture_identity(adapter.kind, &adapter.launch_path, &adapter.home, None)
        .map_err(|_| CliAdapterError::IdentityChanged(adapter.kind.label()))?;
    if current != adapter.identity {
        return Err(CliAdapterError::IdentityChanged(adapter.kind.label()));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn capture_test_identity(path: &Path) -> Result<ExecutableIdentity, CliAdapterError> {
    let canonical_path = path
        .canonicalize()
        .map_err(|_| CliAdapterError::UnsafeExecutable("test CLI"))?;
    let metadata = canonical_path
        .metadata()
        .map_err(|_| CliAdapterError::UnsafeExecutable("test CLI"))?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(CliAdapterError::UnsafeExecutable("test CLI"));
    }
    Ok(identity_from(canonical_path, &metadata))
}

fn identity_from(canonical_path: PathBuf, metadata: &Metadata) -> ExecutableIdentity {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;
    ExecutableIdentity {
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

/// A launch path may be a symlink, so the resolved target is what is actually
/// executed and what must be allow-listed. Each CLI has its own set of install
/// roots; nothing outside them is ever run.
fn canonical_target_allowed(kind: CliKind, path: &Path, home: &Path) -> bool {
    match kind {
        CliKind::Codex => {
            path.starts_with(home.join(".codex/packages/standalone"))
                || path.starts_with("/Applications/ChatGPT.app/Contents/Resources")
                || path.starts_with("/Applications/Codex.app/Contents/Resources")
                || path.starts_with("/opt/homebrew/Cellar/codex")
                || path.starts_with("/usr/local/Cellar/codex")
                || path == Path::new("/opt/homebrew/bin/codex")
                || path == Path::new("/usr/local/bin/codex")
        }
        CliKind::Claude => {
            // Native installer layout: ~/.local/share/claude/versions/<version>.
            path.starts_with(home.join(".local/share/claude/versions"))
                || path.starts_with(home.join(".claude/local"))
                // npm global installs live under a Node prefix that varies by
                // version manager, so the package path itself is the anchor.
                || is_claude_code_package(path, home)
                || path.starts_with("/opt/homebrew/Cellar/claude")
                || path.starts_with("/usr/local/Cellar/claude")
                || path == Path::new("/opt/homebrew/bin/claude")
                || path == Path::new("/usr/local/bin/claude")
        }
    }
}

/// True for `<prefix>/node_modules/@anthropic-ai/claude-code/...` inside the
/// user's home. The package directory must appear as consecutive path
/// components so a lookalike directory name cannot satisfy the check.
fn is_claude_code_package(path: &Path, home: &Path) -> bool {
    if !path.starts_with(home) {
        return false;
    }
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components
        .windows(3)
        .any(|window| window == ["node_modules", "@anthropic-ai", "claude-code"])
}

fn is_native_executable(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut magic = [0_u8; 4];
    if std::io::Read::read_exact(&mut file, &mut magic).is_err() {
        return false;
    }
    matches!(
        magic,
        [0x7f, b'E', b'L', b'F']
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
    ) || magic[..2] == *b"MZ"
}

pub(super) fn canonical_directory(path: &Path) -> Option<PathBuf> {
    path.canonicalize().ok().filter(|path| path.is_dir())
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
