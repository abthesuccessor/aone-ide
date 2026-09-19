use std::path::{Path, PathBuf};

use crate::{
    error::{AoneError, AoneResult},
    scanner::{canonicalize_relative_file, read_source_file},
    state::WorkspaceContext,
};

pub(super) const MAX_EDITABLE_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RELATIVE_PATH_BYTES: usize = 16 * 1024;

#[derive(Debug)]
pub(super) struct VerifiedDocument {
    pub(super) path: PathBuf,
    pub(super) language: String,
}

pub(super) fn verify_document_precondition(
    context: &WorkspaceContext,
    relative_path: &str,
    expected_content_hash: &str,
) -> AoneResult<VerifiedDocument> {
    validate_relative_path(relative_path)?;
    validate_content_hash(expected_content_hash)?;
    let indexed = context
        .store
        .lock()
        .get_file(relative_path)?
        .ok_or_else(|| AoneError::InvalidRequest("file is not in the workspace index".into()))?;
    let path = canonicalize_relative_file(&context.root, Path::new(relative_path))?;
    let current = read_source_file(&path)?;
    let current_hash = content_hash(&current);
    if current_hash != expected_content_hash {
        return Err(AoneError::InvalidRequest(
            "file changed on disk; reload it before formatting or saving".into(),
        ));
    }
    Ok(VerifiedDocument {
        path,
        language: indexed.language,
    })
}

pub(super) fn validate_source_content(content: &str) -> AoneResult<()> {
    if content.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(AoneError::FileTooLarge);
    }
    if content.as_bytes().contains(&0) {
        return Err(AoneError::BinaryFile);
    }
    Ok(())
}

pub(super) fn validate_format_options(tab_size: u8, print_width: u16) -> AoneResult<()> {
    if !(1..=16).contains(&tab_size) {
        return Err(AoneError::InvalidRequest(
            "tabSize must be between 1 and 16".into(),
        ));
    }
    if !(40..=500).contains(&print_width) {
        return Err(AoneError::InvalidRequest(
            "printWidth must be between 40 and 500".into(),
        ));
    }
    Ok(())
}

pub(super) fn content_hash(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

fn validate_relative_path(relative_path: &str) -> AoneResult<()> {
    if relative_path.trim().is_empty()
        || relative_path.len() > MAX_RELATIVE_PATH_BYTES
        || relative_path.contains(['\0', '\n', '\r'])
    {
        return Err(AoneError::InvalidRequest(
            "invalid workspace-relative file path".into(),
        ));
    }
    Ok(())
}

fn validate_content_hash(hash: &str) -> AoneResult<()> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AoneError::InvalidRequest(
            "expectedContentHash must be a BLAKE3 hex digest".into(),
        ));
    }
    Ok(())
}
