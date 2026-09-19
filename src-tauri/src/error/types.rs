use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AoneError {
    #[error("no workspace is open")]
    WorkspaceNotOpen,
    #[error("workspace path is not a readable directory: {0}")]
    InvalidWorkspace(PathBuf),
    #[error("path escapes the open workspace")]
    PathEscape,
    #[error("access to this sensitive path is denied")]
    SensitivePath,
    #[error("file is too large to preview")]
    FileTooLarge,
    #[error("binary files cannot be previewed")]
    BinaryFile,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("parser error: {0}")]
    Parser(String),
    #[error("watcher error: {0}")]
    Watcher(#[from] notify::Error),
    #[error("internal task failed: {0}")]
    Task(String),
}

impl Serialize for AoneError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AoneResult<T> = Result<T, AoneError>;
