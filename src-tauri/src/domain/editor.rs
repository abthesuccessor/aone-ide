use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteWorkspaceFileRequest {
    pub workspace_id: String,
    pub relative_path: String,
    pub content: String,
    pub expected_content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatDocumentRequest {
    pub workspace_id: String,
    pub relative_path: String,
    pub content: String,
    pub expected_content_hash: String,
    pub tab_size: u8,
    pub insert_spaces: bool,
    pub print_width: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormatterCapability {
    pub language: String,
    pub formatter: String,
    pub available: bool,
    pub external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormatDocumentResult {
    pub content: String,
    pub formatter: String,
    pub changed: bool,
    pub used_external_tool: bool,
}
