use std::{fs::OpenOptions, io::Write};

use tauri::{AppHandle, State};

use crate::{
    ai::SecretState,
    commands::open_workspace_at_path,
    domain::{
        CloneGithubRepositoryRequest, CreateDocumentsProjectRequest, GitOnboardingReport,
        InspectGitOnboardingRequest, ProjectBootstrapAction, ProjectBootstrapResult,
    },
    error::{AoneError, AoneResult},
    runner::RuntimeState,
    state::AppState,
    terminal::TerminalState,
};

use super::{
    confirmation::{confirm_clone, confirm_create, confirm_git_inspection},
    inspection::inspect_git,
    paths::{
        create_attested_directory, direct_child, documents_directory, ensure_absent,
        ensure_created_directory, ensure_identity,
    },
    process::{clone_repository, initialize_repository},
    state::OnboardingState,
    validation::{validate_github_repository, validate_project_name, validate_workspace_id},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[tauri::command]
pub async fn clone_github_repository(
    request: CloneGithubRepositoryRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
    terminal: State<'_, TerminalState>,
    onboarding: State<'_, OnboardingState>,
) -> AoneResult<ProjectBootstrapResult> {
    let _onboarding = onboarding.reserve()?;
    let _workspace_operation = workspaces.try_reserve_workspace_operation()?;
    let repository = validate_github_repository(&request.repository_url)?;
    let destination_name = validate_project_name(&request.destination_name, "destinationName")?;
    let (documents, documents_identity) = documents_directory()?;
    let target = direct_child(&documents, &destination_name)?;
    if !confirm_clone(&app, &repository.canonical_url, &target).await? {
        return Err(AoneError::InvalidRequest(
            "GitHub clone was cancelled".into(),
        ));
    }
    ensure_identity(&documents, &documents_identity)?;
    ensure_absent(&target)?;
    let target_identity = create_attested_directory(&target)?;
    ensure_identity(&documents, &documents_identity)?;
    ensure_created_directory(&documents, &target, &target_identity)?;
    if let Err(error) = clone_repository(&target, &repository.canonical_url).await {
        ensure_created_directory(&documents, &target, &target_identity)?;
        return Err(AoneError::Task(format!(
            "{error}. A partial folder remains at Documents/{destination_name}; Aone did not remove or follow it"
        )));
    }
    ensure_identity(&documents, &documents_identity)?;
    ensure_created_directory(&documents, &target, &target_identity)?;
    let workspace =
        open_installed_workspace(target, app, &workspaces, &runtime, &secrets, &terminal).await?;
    Ok(ProjectBootstrapResult {
        action: ProjectBootstrapAction::Clone,
        workspace,
        destination_hint: format!("Documents/{destination_name}"),
        repository_url: Some(repository.canonical_url),
    })
}

#[tauri::command]
pub async fn create_documents_project(
    request: CreateDocumentsProjectRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
    terminal: State<'_, TerminalState>,
    onboarding: State<'_, OnboardingState>,
) -> AoneResult<ProjectBootstrapResult> {
    let _onboarding = onboarding.reserve()?;
    let _workspace_operation = workspaces.try_reserve_workspace_operation()?;
    let project_name = validate_project_name(&request.project_name, "projectName")?;
    let (documents, documents_identity) = documents_directory()?;
    let target = direct_child(&documents, &project_name)?;
    if !confirm_create(&app, &target, request.initialize_git).await? {
        return Err(AoneError::InvalidRequest(
            "project creation was cancelled".into(),
        ));
    }
    ensure_identity(&documents, &documents_identity)?;
    ensure_absent(&target)?;
    let target_identity = create_attested_directory(&target)?;
    ensure_identity(&documents, &documents_identity)?;
    ensure_created_directory(&documents, &target, &target_identity)?;
    write_readme(&target, &project_name)?;
    if request.initialize_git
        && let Err(error) = initialize_repository(&target).await
    {
        ensure_created_directory(&documents, &target, &target_identity)?;
        return Err(AoneError::Task(format!(
            "{error}. The new project remains at Documents/{project_name}; no existing path was changed"
        )));
    }
    ensure_identity(&documents, &documents_identity)?;
    ensure_created_directory(&documents, &target, &target_identity)?;
    let workspace =
        open_installed_workspace(target, app, &workspaces, &runtime, &secrets, &terminal).await?;
    Ok(ProjectBootstrapResult {
        action: ProjectBootstrapAction::Create,
        workspace,
        destination_hint: format!("Documents/{project_name}"),
        repository_url: None,
    })
}

#[tauri::command]
pub async fn inspect_git_onboarding(
    request: InspectGitOnboardingRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    onboarding: State<'_, OnboardingState>,
) -> AoneResult<Option<GitOnboardingReport>> {
    validate_workspace_id(&request.workspace_id)?;
    let _onboarding = onboarding.reserve()?;
    let _workspace_operation = workspaces.try_reserve_workspace_operation()?;
    let context = workspaces.workspace()?;
    ensure_workspace_match(&context.id, &request.workspace_id)?;
    if !confirm_git_inspection(&app, &context.name, &context.root).await? {
        return Ok(None);
    }
    workspaces.while_workspace_current(&context.id, || Ok(()))?;
    let report = inspect_git(context.id.clone(), &context.root)?;
    workspaces.while_workspace_current(&context.id, || Ok(()))?;
    Ok(Some(report))
}

async fn open_installed_workspace(
    path: std::path::PathBuf,
    app: AppHandle,
    workspaces: &AppState,
    runtime: &RuntimeState,
    secrets: &SecretState,
    terminal: &TerminalState,
) -> AoneResult<crate::domain::WorkspaceSummary> {
    terminal.begin_workspace_switch()?;
    match open_workspace_at_path(path, app, workspaces, runtime, secrets).await {
        Ok(summary) => {
            terminal.complete_workspace_switch();
            Ok(summary)
        }
        Err(error) => {
            terminal.cancel_workspace_switch();
            Err(error)
        }
    }
}

fn write_readme(target: &std::path::Path, project_name: &str) -> AoneResult<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(target.join("README.md"))?;
    writeln!(file, "# {project_name}\n")?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn ensure_workspace_match(actual: &str, requested: &str) -> AoneResult<()> {
    if actual != requested {
        return Err(AoneError::InvalidRequest(
            "workspaceId does not match the open workspace".into(),
        ));
    }
    Ok(())
}
