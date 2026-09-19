use serde::{Deserialize, Serialize};

use super::EvidenceKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigurationStatus {
    pub configured: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub inference_available: bool,
}

/// Renderer input for an explicitly selected hosted provider. This type is
/// intentionally deserialize-only and does not implement `Debug` or
/// `Serialize`, so credential-bearing input cannot be accidentally returned or
/// formatted by the backend.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureHostedAiRequest {
    pub provider: String,
    pub api_key: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureOllamaRequest {
    pub endpoint: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureAiCliRequest {
    pub adapter: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCliAdapterStatus {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub readiness: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExplainRequest {
    pub workspace_id: String,
    pub question: String,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub runtime_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExplainProjectEnvironmentRequest {
    pub workspace_id: String,
    pub report_id: String,
}

/// One stateless Project Agent question. The native backend owns the project
/// evidence and provider instructions; the renderer supplies only the bounded
/// engineer question and the identity of an already-approved environment
/// report.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiProjectAgentRequest {
    pub workspace_id: String,
    pub report_id: String,
    pub question: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceReference {
    pub kind: String,
    pub id: String,
    pub label: String,
    pub evidence: EvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExplanation {
    pub answer: String,
    pub evidence: Vec<EvidenceReference>,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiProjectAgentErrorCode {
    Cancelled,
    NotConfigured,
    InvalidRequest,
    WorkspaceChanged,
    ProviderFailed,
}

/// A typed outcome is returned for both expected failures and successful
/// answers so the renderer can present honest idle/thinking/complete/error
/// states without parsing native error strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AiProjectAgentResponse {
    Completed {
        answer: String,
        evidence: Vec<EvidenceReference>,
        model: String,
    },
    Error {
        code: AiProjectAgentErrorCode,
        message: String,
        retryable: bool,
    },
}
