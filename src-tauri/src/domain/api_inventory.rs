use serde::{Deserialize, Serialize};

use super::{EvidenceKind, SourceLocation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiEndpointRole {
    Producer,
    Consumer,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiProtocol {
    Http,
    WebSocket,
    Event,
    Rpc,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiGroupingBasis {
    OpenApiTag,
    SourcePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointGrouping {
    pub key: String,
    pub segments: Vec<String>,
    pub basis: ApiGroupingBasis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointHandler {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLocation>,
    pub evidence: EvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiClientCoverage {
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_source: Option<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpoint {
    pub id: String,
    pub label: String,
    pub method: String,
    pub path: String,
    pub role: ApiEndpointRole,
    pub protocol: ApiProtocol,
    pub source_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub evidence: EvidenceKind,
    pub grouping: ApiEndpointGrouping,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<ApiEndpointHandler>,
    pub client_coverage: ApiClientCoverage,
    pub occurrence_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiInventoryCounts {
    pub producer: usize,
    pub consumer_only: usize,
    pub client_covered: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListApiEndpointsRequest {
    pub workspace_id: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointPage {
    pub workspace_id: String,
    pub endpoints: Vec<ApiEndpoint>,
    pub total: usize,
    pub indexed_total: usize,
    pub returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub truncated: bool,
    pub counts: ApiInventoryCounts,
}
