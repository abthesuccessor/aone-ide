use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchRequest {
    pub workspace_id: String,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceSearchMatchKind {
    Content,
    Path,
    Symbol,
    Endpoint,
    Event,
    Heading,
    Sentence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchMatch {
    pub key: String,
    pub relative_path: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub preview: String,
    pub match_text: String,
    pub kind: WorkspaceSearchMatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchResult {
    pub query: String,
    pub matches: Vec<WorkspaceSearchMatch>,
    pub total_matches: usize,
    pub truncated: bool,
    pub indexed_file_count: usize,
}
