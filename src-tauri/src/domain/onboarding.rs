use serde::{Deserialize, Serialize};

use super::WorkspaceSummary;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneGithubRepositoryRequest {
    pub repository_url: String,
    pub destination_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentsProjectRequest {
    pub project_name: String,
    pub initialize_git: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectGitOnboardingRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapResult {
    pub action: ProjectBootstrapAction,
    pub workspace: WorkspaceSummary,
    pub destination_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectBootstrapAction {
    Clone,
    Create,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOnboardingReport {
    pub workspace_id: String,
    pub inspected_at: String,
    pub is_repository: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<GitOnboardingIdentity>,
    pub remotes: Vec<GitOnboardingRemote>,
    pub public_keys: Vec<GitOnboardingPublicKey>,
    pub private_keys_read: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOnboardingIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub source: GitIdentitySource,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitIdentitySource {
    Local,
    Global,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOnboardingRemote {
    pub name: String,
    pub display_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_repo: Option<String>,
    pub transport: GitRemoteTransport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitRemoteTransport {
    Https,
    Ssh,
    Other,
    Local,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOnboardingPublicKey {
    pub file_name: String,
    pub key_type: String,
    pub fingerprint: String,
}
