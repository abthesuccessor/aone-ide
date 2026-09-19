use crate::{analyzer::AnalyzedSource, domain::WorkspaceFile};

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub file: WorkspaceFile,
    /// Source is retained only until the transactional derived indexes are
    /// updated. The SQLite search projection is contentless and does not keep
    /// a second plaintext copy of the workspace file.
    pub source: String,
    pub analysis: AnalyzedSource,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub files: Vec<IndexedFile>,
}
