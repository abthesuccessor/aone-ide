use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectProjectEnvironmentRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEnvironmentReport {
    pub report_id: String,
    pub workspace_id: String,
    pub inspected_at: String,
    pub version_probe_approved: bool,
    pub stacks: Vec<ProjectStack>,
    pub tools: Vec<ProjectTool>,
    pub recommendations: Vec<ProjectEnvironmentRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStack {
    pub id: String,
    pub label: String,
    pub confidence: ProjectStackConfidence,
    pub evidence: Vec<ProjectEnvironmentEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEnvironmentEvidence {
    pub relative_path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectStackConfidence {
    Confirmed,
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTool {
    pub id: String,
    pub label: String,
    pub category: String,
    pub required: bool,
    pub status: ProjectToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    #[serde(default)]
    pub alternate_canonical_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub version_args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_error: Option<String>,
    #[serde(default)]
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectToolStatus {
    Available,
    Missing,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEnvironmentRecommendation {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub kind: ProjectEnvironmentRecommendationKind,
    pub severity: ProjectEnvironmentRecommendationSeverity,
    #[serde(default)]
    pub evidence: Vec<ProjectEnvironmentEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_profile_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectEnvironmentRecommendationKind {
    RunProfile,
    Tool,
    Environment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectEnvironmentRecommendationSeverity {
    Recommended,
    Optional,
    Warning,
}
