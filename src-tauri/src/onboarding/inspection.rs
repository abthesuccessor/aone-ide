use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Take},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose};
use directories::UserDirs;
use sha2::{Digest, Sha256};

use crate::{
    domain::{
        GitIdentitySource, GitOnboardingIdentity, GitOnboardingPublicKey, GitOnboardingRemote,
        GitOnboardingReport, GitRemoteTransport,
    },
    error::{AoneError, AoneResult},
};

#[cfg(unix)]
use std::{
    ffi::{CStr, CString},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::OpenOptionsExt,
    },
};

const CONFIG_LIMIT: u64 = 256 * 1024;
const KEY_LIMIT: u64 = 16 * 1024;
const MAX_REMOTES: usize = 32;
const MAX_PUBLIC_KEYS: usize = 16;

#[derive(Debug, Default)]
pub(super) struct ParsedConfig {
    pub(super) name: Option<String>,
    pub(super) email: Option<String>,
    pub(super) remotes: BTreeMap<String, String>,
}

pub(super) fn inspect_git(workspace_id: String, root: &Path) -> AoneResult<GitOnboardingReport> {
    let git_directory = canonical_real_directory(&root.join(".git"))
        .filter(|directory| directory.parent() == Some(root));
    let local = git_directory
        .as_deref()
        .and_then(|directory| read_config(&directory.join("config"), CONFIG_LIMIT).ok())
        .map(|bytes| parse_config(&bytes))
        .unwrap_or_default();
    let home = canonical_home();
    let global = home
        .as_deref()
        .map(read_global_identity)
        .unwrap_or_default();
    let identity = select_identity(&local, &global);
    let branch = git_directory
        .as_deref()
        .and_then(|directory| read_config(&directory.join("HEAD"), 1024).ok())
        .and_then(|bytes| parse_branch(&bytes));
    let remotes = local
        .remotes
        .into_iter()
        .take(MAX_REMOTES)
        .map(|(name, url)| sanitize_remote(&name, &url))
        .collect();
    let public_keys = home.as_deref().map(read_public_keys).unwrap_or_default();
    Ok(GitOnboardingReport {
        workspace_id,
        inspected_at: timestamp_millis(),
        is_repository: git_directory.is_some(),
        branch,
        identity,
        remotes,
        public_keys,
        private_keys_read: false,
    })
}

fn read_global_identity(home: &Path) -> ParsedConfig {
    let candidates = [home.join(".config/git/config"), home.join(".gitconfig")];
    let mut merged = ParsedConfig::default();
    for path in candidates {
        let Ok(bytes) = read_config(&path, CONFIG_LIMIT) else {
            continue;
        };
        let parsed = parse_config(&bytes);
        if parsed.name.is_some() {
            merged.name = parsed.name;
        }
        if parsed.email.is_some() {
            merged.email = parsed.email;
        }
    }
    merged
}

fn select_identity(local: &ParsedConfig, global: &ParsedConfig) -> Option<GitOnboardingIdentity> {
    if local.name.is_some() || local.email.is_some() {
        return Some(GitOnboardingIdentity {
            name: local.name.clone(),
            email: local.email.clone(),
            source: GitIdentitySource::Local,
        });
    }
    if global.name.is_some() || global.email.is_some() {
        return Some(GitOnboardingIdentity {
            name: global.name.clone(),
            email: global.email.clone(),
            source: GitIdentitySource::Global,
        });
    }
    None
}

pub(super) fn parse_config(bytes: &[u8]) -> ParsedConfig {
    let text = String::from_utf8_lossy(bytes);
    let mut parsed = ParsedConfig::default();
    let mut section = String::new();
    for raw in text.lines().take(4_096) {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let Some(value) = clean_value(strip_inline_comment(value), 2_048) else {
            continue;
        };
        if section == "user" && key == "name" {
            parsed.name = clean_value(&value, 200);
        } else if section == "user" && key == "email" {
            parsed.email = clean_value(&value, 240);
        } else if key == "url"
            && let Some(name) = remote_name(&section)
        {
            parsed.remotes.insert(name, value);
        }
    }
    parsed
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
            continue;
        }
        if !quoted && matches!(character, '#' | ';') {
            return &value[..index];
        }
    }
    value
}

fn remote_name(section: &str) -> Option<String> {
    let value = section.strip_prefix("remote ")?.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    clean_value(value, 64).filter(|name| {
        !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    })
}

pub(super) fn sanitize_remote(name: &str, raw_url: &str) -> GitOnboardingRemote {
    let clean = clean_value(raw_url, 2_048).unwrap_or_default();
    let without_suffix = clean.split(['?', '#']).next().unwrap_or_default();
    if let Some(rest) = without_suffix.strip_prefix("https://") {
        return network_remote(name, rest, GitRemoteTransport::Https, "https");
    }
    if let Some(rest) = without_suffix.strip_prefix("ssh://") {
        return network_remote(name, rest, GitRemoteTransport::Ssh, "ssh");
    }
    if let Some((authority, path)) = without_suffix.split_once(':')
        && authority.contains('@')
        && !authority.contains('/')
    {
        return network_remote(
            name,
            &format!("{authority}/{path}"),
            GitRemoteTransport::Ssh,
            "ssh",
        );
    }
    if without_suffix.starts_with(['/', '.', '~']) || without_suffix.starts_with("file://") {
        return GitOnboardingRemote {
            name: clean_name(name),
            display_url: "[local repository]".into(),
            host: None,
            owner_repo: None,
            transport: GitRemoteTransport::Local,
        };
    }
    let scheme = without_suffix
        .split_once("://")
        .map(|(scheme, _)| scheme)
        .filter(|scheme| scheme.chars().all(|value| value.is_ascii_alphanumeric()))
        .unwrap_or("other");
    GitOnboardingRemote {
        name: clean_name(name),
        display_url: format!("{scheme}://[redacted]"),
        host: None,
        owner_repo: None,
        transport: GitRemoteTransport::Other,
    }
}

fn network_remote(
    name: &str,
    rest: &str,
    transport: GitRemoteTransport,
    scheme: &str,
) -> GitOnboardingRemote {
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let raw_host = authority
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let host = clean_value(&raw_host, 253).filter(|value| {
        value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-".contains(character))
    });
    let owner_repo = host
        .as_deref()
        .and_then(|host| github_owner_repo(host, path));
    let display_path = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };
    let display_host = host.as_deref().unwrap_or("[redacted]");
    GitOnboardingRemote {
        name: clean_name(name),
        display_url: clean_value(&format!("{scheme}://{display_host}{display_path}"), 2_048)
            .unwrap_or_else(|| format!("{scheme}://[redacted]")),
        host,
        owner_repo,
        transport,
    }
}

fn github_owner_repo(host: &str, path: &str) -> Option<String> {
    if host != "github.com" {
        return None;
    }
    let mut parts = path.trim_matches('/').split('/');
    let owner = parts.next()?;
    let raw_repo = parts.next()?;
    let repo = raw_repo.strip_suffix(".git").unwrap_or(raw_repo);
    if owner.is_empty()
        || owner.len() > 39
        || repo.is_empty()
        || repo.len() > 100
        || !owner
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        || !repo
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

#[cfg(unix)]
pub(super) fn read_public_keys(home: &Path) -> Vec<GitOnboardingPublicKey> {
    let Some(home_directory) = open_directory(home) else {
        return Vec::new();
    };
    let Some(ssh_directory) = open_relative_directory(&home_directory, ".ssh") else {
        return Vec::new();
    };
    let mut names = public_key_names(&ssh_directory);
    names.sort();
    names
        .into_iter()
        .take(MAX_PUBLIC_KEYS)
        .filter_map(|file_name| {
            let bytes = read_relative_public_key(&ssh_directory, &file_name)?;
            parse_public_key(file_name, &bytes)
        })
        .collect()
}

#[cfg(not(unix))]
pub(super) fn read_public_keys(_home: &Path) -> Vec<GitOnboardingPublicKey> {
    Vec::new()
}

fn parse_public_key(file_name: String, bytes: &[u8]) -> Option<GitOnboardingPublicKey> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut fields = text.split_whitespace();
    let key_type = fields.next()?;
    if key_type.len() > 64
        || !(key_type.starts_with("ssh-")
            || key_type.starts_with("ecdsa-")
            || key_type.starts_with("sk-"))
        || !key_type
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@._+-".contains(character))
    {
        return None;
    }
    let decoded = general_purpose::STANDARD.decode(fields.next()?).ok()?;
    if decoded.is_empty() || decoded.len() > KEY_LIMIT as usize {
        return None;
    }
    let digest = Sha256::digest(decoded);
    Some(GitOnboardingPublicKey {
        file_name,
        key_type: key_type.into(),
        fingerprint: format!("SHA256:{}", general_purpose::STANDARD_NO_PAD.encode(digest)),
    })
}

#[cfg(unix)]
fn open_directory(path: &Path) -> Option<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let file = options.open(path).ok()?;
    file.metadata().ok()?.is_dir().then_some(file)
}

#[cfg(unix)]
fn open_relative_directory(parent: &File, name: &str) -> Option<File> {
    let name = CString::new(name).ok()?;
    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return None;
    }
    let file = unsafe { File::from_raw_fd(descriptor) };
    file.metadata().ok()?.is_dir().then_some(file)
}

#[cfg(unix)]
fn public_key_names(directory: &File) -> Vec<String> {
    let descriptor = unsafe { libc::dup(directory.as_raw_fd()) };
    if descriptor < 0 {
        return Vec::new();
    }
    let stream = unsafe { libc::fdopendir(descriptor) };
    if stream.is_null() {
        let _ = unsafe { libc::close(descriptor) };
        return Vec::new();
    }
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let Ok(name) = name.to_str() else {
            continue;
        };
        if name.ends_with(".pub")
            && name.len() <= 255
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        {
            names.push(name.to_owned());
        }
    }
    let _ = unsafe { libc::closedir(stream) };
    names
}

#[cfg(unix)]
fn read_relative_public_key(directory: &File, file_name: &str) -> Option<Vec<u8>> {
    let name = CString::new(file_name).ok()?;
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if descriptor < 0 {
        return None;
    }
    let file = unsafe { File::from_raw_fd(descriptor) };
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > KEY_LIMIT {
        return None;
    }
    read_limited(file.take(KEY_LIMIT + 1), KEY_LIMIT).ok()
}

fn parse_branch(bytes: &[u8]) -> Option<String> {
    let value = clean_value(&String::from_utf8_lossy(bytes), 240)?;
    if let Some(branch) = value.strip_prefix("ref: refs/heads/") {
        return clean_value(branch, 200);
    }
    value
        .chars()
        .all(|character| character.is_ascii_hexdigit())
        .then(|| value.chars().take(12).collect())
}

fn read_config(path: &Path, limit: u64) -> AoneResult<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(AoneError::InvalidRequest(
            "Git metadata file is not a bounded regular file".into(),
        ));
    }
    read_limited(file.take(limit + 1), limit)
}

fn read_limited(mut file: Take<File>, limit: u64) -> AoneResult<Vec<u8>> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(AoneError::InvalidRequest(
            "Git metadata file exceeded its read limit".into(),
        ));
    }
    Ok(bytes)
}

fn real_directory(path: &Path) -> Option<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    (!metadata.file_type().is_symlink() && metadata.is_dir()).then(|| path.to_path_buf())
}

fn canonical_real_directory(path: &Path) -> Option<PathBuf> {
    let directory = real_directory(path)?;
    directory.canonicalize().ok()
}

fn canonical_home() -> Option<PathBuf> {
    let configured = UserDirs::new()?.home_dir().to_path_buf();
    canonical_real_directory(&configured)
}

fn clean_name(value: &str) -> String {
    clean_value(value, 64).unwrap_or_else(|| "remote".into())
}

fn clean_value(value: &str, limit: usize) -> Option<String> {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(trimmed);
    let clean = trimmed
        .chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect::<String>();
    (!clean.is_empty()).then_some(clean)
}

fn timestamp_millis() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
