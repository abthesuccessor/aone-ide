use std::path::{Component, Path, PathBuf};

use crate::error::{AoneError, AoneResult};

pub(super) fn normalize_event_path(root: &Path, path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    if let Ok(canonical) = absolute.canonicalize() {
        return canonical;
    }

    let mut cursor = absolute.as_path();
    let mut missing = Vec::new();
    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            return absolute;
        };
        missing.push(name.to_owned());
        let Some(parent) = cursor.parent() else {
            return absolute;
        };
        cursor = parent;
    }
    let Ok(mut normalized) = cursor.canonicalize() else {
        return absolute;
    };
    for component in missing.into_iter().rev() {
        normalized.push(component);
    }
    normalized
}

pub(super) fn lexical_relative(root: &Path, path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    absolute.strip_prefix(root).ok().map(Path::to_path_buf)
}

pub(super) fn relative_components(path: &Path) -> AoneResult<String> {
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
