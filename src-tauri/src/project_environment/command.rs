use std::collections::BTreeMap;

use tauri::{AppHandle, State};
use uuid::Uuid;

use super::{
    analysis::{ReportIdentity, build_report, derive_project_facts},
    config_hints::collect_project_hints,
    confirmation::{confirm_environment_inspection, confirm_version_probes},
    discovery::discover_tools,
    markers::collect_marker_evidence,
    probe::{build_probe_plan, run_probe_plan},
    state::ProjectEnvironmentState,
    time::timestamp_millis,
};
use crate::{
    domain::{InspectProjectEnvironmentRequest, ProjectEnvironmentReport},
    error::{AoneError, AoneResult},
    run_profiles::detect_profiles,
    state::AppState,
};

/// Reads the latest already-approved report for the currently open workspace.
///
/// Unlike `inspect_project_environment`, this command performs no discovery,
/// file reads, PATH inspection, or process execution and therefore does not
/// show an inspection consent dialog. The renderer can use it when reopening
/// the Project Agent surface; an explicit inspection remains the only way to
/// create or refresh a report.
#[tauri::command]
pub fn get_project_environment_report(
    request: InspectProjectEnvironmentRequest,
    workspaces: State<'_, AppState>,
    environments: State<'_, ProjectEnvironmentState>,
) -> AoneResult<Option<ProjectEnvironmentReport>> {
    validate_workspace_id(&request.workspace_id)?;
    let context = workspaces.workspace()?;
    if context.id != request.workspace_id {
        return Err(AoneError::InvalidRequest(
            "workspaceId does not match the open workspace".into(),
        ));
    }

    workspaces.while_workspace_current(&request.workspace_id, || {
        Ok(environments.report_for_workspace(&request.workspace_id))
    })
}

#[tauri::command]
pub async fn inspect_project_environment(
    request: InspectProjectEnvironmentRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    environments: State<'_, ProjectEnvironmentState>,
) -> AoneResult<Option<ProjectEnvironmentReport>> {
    validate_workspace_id(&request.workspace_id)?;
    let _reservation = environments.reserve_inspection()?;
    let context = workspaces.workspace()?;
    if context.id != request.workspace_id {
        return Err(AoneError::InvalidRequest(
            "workspaceId does not match the open workspace".into(),
        ));
    }

    // This is deliberately the first local inspection decision. No PATH,
    // executable location, project manifest/build/config source, or language
    // summary is read by this command before the user accepts this dialog.
    if !confirm_environment_inspection(&app, &context.name, &context.root).await? {
        return Ok(None);
    }
    ensure_workspace_current(&workspaces, &context.id)?;

    let languages = context.summary.read().languages.clone();
    let profiles = detect_profiles(&context.root, &context.id)?;
    let (markers, hints) = {
        let store = context.store.lock();
        (
            collect_marker_evidence(&store)?,
            collect_project_hints(&context.root, &store)?,
        )
    };
    let git_workspace = std::fs::symlink_metadata(context.root.join(".git")).is_ok();
    let discovered = discover_tools(&context.root)?;
    let facts = derive_project_facts(&languages, &profiles, &markers, git_workspace);
    let probe_plan = build_probe_plan(&discovered, &facts.probe_tool_ids);

    ensure_workspace_current(&workspaces, &context.id)?;
    let version_probe_approved = confirm_version_probes(&app, &probe_plan).await?;
    let probes = if version_probe_approved {
        ensure_workspace_current(&workspaces, &context.id)?;
        run_probe_plan(&probe_plan).await
    } else {
        BTreeMap::new()
    };
    ensure_workspace_current(&workspaces, &context.id)?;

    let report = build_report(
        ReportIdentity {
            report_id: format!("project-environment:{}", Uuid::now_v7()),
            workspace_id: context.id.clone(),
            inspected_at: timestamp_millis(),
            version_probe_approved,
        },
        facts,
        &profiles,
        &discovered,
        &probes,
        &hints,
    );
    environments.store_report(report.clone());
    Ok(Some(report))
}

fn validate_workspace_id(workspace_id: &str) -> AoneResult<()> {
    if workspace_id.is_empty()
        || workspace_id.len() > 200
        || workspace_id.chars().any(char::is_control)
    {
        return Err(AoneError::InvalidRequest(
            "workspaceId is missing or invalid".into(),
        ));
    }
    Ok(())
}

fn ensure_workspace_current(state: &AppState, workspace_id: &str) -> AoneResult<()> {
    state.while_workspace_current(workspace_id, || Ok(()))
}

#[cfg(test)]
pub(super) fn validate_workspace_id_for_test(workspace_id: &str) -> AoneResult<()> {
    validate_workspace_id(workspace_id)
}
