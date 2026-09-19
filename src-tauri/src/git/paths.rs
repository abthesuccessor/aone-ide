use std::path::{Component, Path, PathBuf};

use crate::{
    error::{AoneError, AoneResult},
    scanner::is_hard_denied,
};

const MAX_PATHS: usize = 200;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_TOTAL_PATH_BYTES: usize = 64 * 1024;

pub(super) fn validate_paths(root: &Path, raw_paths: &[String]) -> AoneResult<Vec<String>> {
    if raw_paths.is_empty() || raw_paths.len() > MAX_PATHS {
        return Err(AoneError::InvalidRequest(format!(
            "select between 1 and {MAX_PATHS} Git paths"
        )));
    }
    let mut total = 0_usize;
    let mut paths = Vec::with_capacity(raw_paths.len());
    for raw in raw_paths {
        total = total.saturating_add(raw.len());
        if raw.is_empty() || raw.len() > MAX_PATH_BYTES || total > MAX_TOTAL_PATH_BYTES {
            return Err(AoneError::InvalidRequest(
                "Git path selection is empty or too large".into(),
            ));
        }
        paths.push(validate_path(root, raw)?);
    }
    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}

pub(super) fn validate_path(root: &Path, raw: &str) -> AoneResult<String> {
    if raw.is_empty() || raw.len() > MAX_PATH_BYTES {
        return Err(AoneError::InvalidRequest(
            "Git path is empty or too large".into(),
        ));
    }
    if raw.chars().any(is_unsafe_display_character) {
        return Err(AoneError::InvalidRequest(
            "Git paths cannot contain control or direction-override characters".into(),
        ));
    }
    let relative = Path::new(raw);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || is_hard_denied(relative)
    {
        return Err(AoneError::SensitivePath);
    }

    inspect_existing_prefixes(root, relative)?;
    let candidate = root.join(relative);
    if let Ok(metadata) = candidate.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            return Err(AoneError::SensitivePath);
        }
        if metadata.is_dir() {
            return Err(AoneError::InvalidRequest(
                "Git actions require file paths, not directories".into(),
            ));
        }
        let canonical = candidate.canonicalize()?;
        if !canonical.starts_with(root) {
            return Err(AoneError::PathEscape);
        }
    }
    Ok(relative_to_slashes(relative))
}

fn inspect_existing_prefixes(root: &Path, relative: &Path) -> AoneResult<()> {
    let mut candidate = PathBuf::from(root);
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(AoneError::PathEscape);
        };
        candidate.push(value);
        let Ok(metadata) = candidate.symlink_metadata() else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            return Err(AoneError::SensitivePath);
        }
        if !candidate.canonicalize()?.starts_with(root) {
            return Err(AoneError::PathEscape);
        }
    }
    Ok(())
}

fn relative_to_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn is_unsafe_display_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
