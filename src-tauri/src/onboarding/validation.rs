use crate::error::{AoneError, AoneResult};

const GITHUB_PREFIX: &str = "https://github.com/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GithubRepository {
    pub(super) canonical_url: String,
    pub(super) owner_repo: String,
}

pub(super) fn validate_github_repository(value: &str) -> AoneResult<GithubRepository> {
    if value.len() > 300 || value.chars().any(|character| character.is_control()) {
        return invalid("repositoryUrl is too long or contains control characters");
    }
    let path = value
        .strip_prefix(GITHUB_PREFIX)
        .ok_or_else(|| invalid_error("repositoryUrl must use https://github.com"))?;
    if path.contains(['?', '#', '@', '\\', '%']) || path.ends_with('/') {
        return invalid("repositoryUrl must not contain credentials, query, fragment, or encoding");
    }
    let mut segments = path.split('/');
    let owner = segments.next().unwrap_or_default();
    let raw_repo = segments.next().unwrap_or_default();
    if segments.next().is_some() || !valid_owner(owner) {
        return invalid("repositoryUrl must contain one valid GitHub owner and repository");
    }
    let repo = raw_repo.strip_suffix(".git").unwrap_or(raw_repo);
    if !valid_repo(repo) {
        return invalid("repositoryUrl contains an invalid GitHub repository name");
    }
    let owner_repo = format!("{owner}/{repo}");
    Ok(GithubRepository {
        canonical_url: format!("{GITHUB_PREFIX}{owner_repo}.git"),
        owner_repo,
    })
}

pub(super) fn validate_project_name(value: &str, field: &str) -> AoneResult<String> {
    if value.is_empty()
        || value.len() > 100
        || matches!(value, "." | "..")
        || value.starts_with('.')
        || value.ends_with(['.', ' '])
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return invalid(&format!(
            "{field} must be 1 to 100 ASCII letters, numbers, dots, dashes, or underscores"
        ));
    }
    Ok(value.to_owned())
}

pub(super) fn validate_workspace_id(value: &str) -> AoneResult<()> {
    if value.is_empty() || value.len() > 200 || value.chars().any(char::is_control) {
        return invalid("workspaceId is missing or invalid");
    }
    Ok(())
}

fn valid_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn valid_repo(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
}

fn invalid<T>(message: &str) -> AoneResult<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: &str) -> AoneError {
    AoneError::InvalidRequest(message.into())
}
