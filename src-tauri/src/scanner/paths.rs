use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use ignore::WalkBuilder;

use crate::{
    analyzer::stable_id,
    error::{AoneError, AoneResult},
};

const DENIED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".aone",
    "node_modules",
    "bower_components",
    "vendor",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    ".docker",
    ".kube",
    ".aws",
    ".azure",
    ".ssh",
    ".gnupg",
    ".terraform",
    ".gradle",
    ".idea",
    ".venv",
    "venv",
    "__pycache__",
    "Pods",
    "DerivedData",
];

const DENIED_FILE_NAMES: &[&str] = &[
    ".npmrc",
    ".pypirc",
    ".netrc",
    "credentials",
    "credentials.json",
    "service-account.json",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".git-credentials",
    "application_default_credentials.json",
];

const DENIED_EXTENSIONS: &[&str] = &[
    "pem", "key", "p12", "pfx", "jks", "keystore", "crt", "cer", "der",
];

pub fn canonical_workspace(path: &Path) -> AoneResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|_| AoneError::InvalidWorkspace(path.to_path_buf()))?;
    if !canonical.is_dir() {
        return Err(AoneError::InvalidWorkspace(path.to_path_buf()));
    }
    Ok(canonical)
}

pub fn workspace_id(root: &Path) -> String {
    stable_id("workspace", &[&root.to_string_lossy()])
}

pub fn canonicalize_relative_file(root: &Path, relative: &Path) -> AoneResult<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(AoneError::PathEscape);
    }
    if is_hard_denied(relative) {
        return Err(AoneError::SensitivePath);
    }

    let candidate = root.join(relative);
    let canonical = candidate.canonicalize()?;
    if !canonical.starts_with(root) {
        return Err(AoneError::PathEscape);
    }
    let lexical_metadata = candidate.symlink_metadata()?;
    if lexical_metadata.file_type().is_symlink() {
        return Err(AoneError::SensitivePath);
    }
    if !canonical.is_file() {
        return Err(AoneError::InvalidRequest(
            "path is not a regular file".into(),
        ));
    }
    Ok(canonical)
}

pub fn relative_path(root: &Path, absolute: &Path) -> AoneResult<String> {
    let relative = absolute
        .strip_prefix(root)
        .map_err(|_| AoneError::PathEscape)?;
    if is_hard_denied(relative) {
        return Err(AoneError::SensitivePath);
    }
    relative_to_string(relative)
}

pub fn is_hard_denied(relative: &Path) -> bool {
    let mut component_names = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return true;
        };
        let text = value.to_string_lossy();
        component_names.push(text.to_ascii_lowercase());
        if DENIED_DIRECTORIES
            .iter()
            .any(|denied| text.eq_ignore_ascii_case(denied))
        {
            return true;
        }
    }

    let Some(name) = relative.file_name().and_then(OsStr::to_str) else {
        return true;
    };
    let lower_name = name.to_ascii_lowercase();
    if lower_name == ".env" || lower_name.starts_with(".env.") {
        return true;
    }
    if DENIED_FILE_NAMES
        .iter()
        .any(|denied| lower_name.eq_ignore_ascii_case(denied))
    {
        return true;
    }
    if lower_name.starts_with("secrets.") || lower_name.starts_with("secret.") {
        return true;
    }
    if component_names
        .windows(2)
        .any(|parts| parts[0] == ".config" && parts[1] == "gcloud")
    {
        return true;
    }
    if component_names.windows(2).any(|parts| {
        parts[0] == ".cargo" && matches!(parts[1].as_str(), "credentials" | "credentials.toml")
    }) {
        return true;
    }
    if lower_name == "terraform.tfstate"
        || lower_name.starts_with("terraform.tfstate.")
        || lower_name.ends_with(".tfstate")
        || lower_name.ends_with(".tfstate.backup")
    {
        return true;
    }
    relative
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            DENIED_EXTENSIONS
                .iter()
                .any(|denied| extension.eq_ignore_ascii_case(denied))
        })
}

pub fn is_discoverable_file(root: &Path, path: &Path) -> bool {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return false;
    }
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    if !canonical.starts_with(root) || !canonical.is_file() {
        return false;
    }
    let Ok(relative) = canonical.strip_prefix(root) else {
        return false;
    };
    if is_hard_denied(relative) {
        return false;
    }

    let target = canonical.clone();
    let root_for_filter = root.to_path_buf();
    let target_for_filter = target.clone();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .parents(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.path() == root_for_filter
                || target_for_filter.starts_with(entry.path())
                    && entry
                        .path()
                        .strip_prefix(&root_for_filter)
                        .is_ok_and(|relative| !is_hard_denied(relative))
        });
    builder
        .build()
        .filter_map(Result::ok)
        .any(|entry| entry.path() == target)
}

pub(super) fn relative_to_string(path: &Path) -> AoneResult<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(AoneError::PathEscape);
        };
        let value = value
            .to_str()
            .ok_or_else(|| AoneError::InvalidRequest("non-UTF-8 paths are not supported".into()))?;
        parts.push(value);
    }
    Ok(parts.join("/"))
}
