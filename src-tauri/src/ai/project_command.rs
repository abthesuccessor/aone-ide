use serde_json::json;
use tauri::{AppHandle, State};
use uuid::Uuid;

use super::{
    project_evidence::{build_project_environment_evidence, prepare_project_agent_question},
    provider::{
        confirm_project_agent_call, confirm_project_environment_call,
        project_agent_provider_request, project_environment_provider_request, request_provider,
    },
    state::SecretState,
};
use crate::{
    domain::{
        AiExplainProjectEnvironmentRequest, AiExplanation, AiProjectAgentErrorCode,
        AiProjectAgentRequest, AiProjectAgentResponse, EvidenceKind,
    },
    error::{AoneError, AoneResult},
    project_environment::ProjectEnvironmentState,
    runner::{RuntimeState, publish_event},
    state::AppState,
};

#[tauri::command]
pub async fn ai_ask_project_agent(
    request: AiProjectAgentRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    environments: State<'_, ProjectEnvironmentState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiProjectAgentResponse> {
    Ok(
        match ask_project_agent(request, app, workspaces, environments, runtime, secrets).await {
            Ok(response) => response,
            Err(error) => project_agent_error_response(error),
        },
    )
}

async fn ask_project_agent(
    request: AiProjectAgentRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    environments: State<'_, ProjectEnvironmentState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiProjectAgentResponse> {
    // Reserve before opening a dialog so rapid submits cannot create
    // overlapping consent prompts or provider calls.
    let _reservation = secrets.reserve_ai_call()?;
    let current = workspaces.workspace()?;
    if current.id != request.workspace_id {
        return Err(AoneError::InvalidRequest(
            "project agent request belongs to another workspace".into(),
        ));
    }
    let report = environments.latest_report(&request.workspace_id, &request.report_id)?;
    let provider = secrets.provider_configuration()?;
    let mut redaction_values = runtime.secret_values_for_redaction();
    redaction_values.extend(provider.secret_values_for_redaction());
    let question = prepare_project_agent_question(&request.question, &redaction_values)?;
    let evidence = build_project_environment_evidence(&report, &runtime)?;

    confirm_project_agent_call(
        &app,
        &provider,
        evidence.references.len(),
        evidence.input.len(),
        question.len(),
    )
    .await?;
    workspaces.while_workspace_current(&request.workspace_id, || Ok(()))?;

    let request_body =
        project_agent_provider_request(provider.kind, &provider.model, &question, &evidence.input);
    let answer =
        request_provider(&provider, &request_body, &redaction_values, &current.root).await?;
    let answer = format!(
        "AI project guidance (inference; no project action was executed):\n\n{}",
        answer.trim()
    );

    workspaces.while_workspace_current(&request.workspace_id, || {
        runtime.with_workspace_current(&current.root, || {
            let request_id = format!("ai:project-agent:{}", Uuid::now_v7());
            let metadata = std::collections::BTreeMap::from([
                ("provider".into(), json!(provider.kind.id())),
                ("model".into(), json!(&provider.model)),
                ("reportId".into(), json!(report.report_id)),
                ("evidenceCount".into(), json!(evidence.references.len())),
                ("questionBytes".into(), json!(question.len())),
            ]);
            let event = runtime.next_event(
                None,
                "ai.projectAgentAnswer",
                "AI-generated project guidance",
                EvidenceKind::Inferred,
                Some(request_id),
                metadata,
            );
            publish_event(&app, &runtime, event);

            AiProjectAgentResponse::Completed {
                answer,
                evidence: evidence.references,
                model: provider.model,
            }
        })
    })
}

fn project_agent_error_response(error: AoneError) -> AiProjectAgentResponse {
    let message = error.to_string();
    let (code, retryable) = match &error {
        AoneError::InvalidRequest(detail) if detail == "AI provider request was cancelled" => {
            (AiProjectAgentErrorCode::Cancelled, true)
        }
        AoneError::InvalidRequest(detail)
            if detail.starts_with("AI explanation is optional; configure") =>
        {
            (AiProjectAgentErrorCode::NotConfigured, false)
        }
        AoneError::WorkspaceNotOpen | AoneError::InvalidWorkspace(_) => {
            (AiProjectAgentErrorCode::WorkspaceChanged, true)
        }
        AoneError::InvalidRequest(detail)
            if detail.contains("workspace")
                && (detail.contains("changed")
                    || detail.contains("another")
                    || detail.contains("stale")) =>
        {
            (AiProjectAgentErrorCode::WorkspaceChanged, true)
        }
        AoneError::InvalidRequest(_) => (AiProjectAgentErrorCode::InvalidRequest, false),
        AoneError::Task(_)
        | AoneError::Io(_)
        | AoneError::Database(_)
        | AoneError::Serialization(_)
        | AoneError::Parser(_)
        | AoneError::Watcher(_)
        | AoneError::PathEscape
        | AoneError::SensitivePath
        | AoneError::FileTooLarge
        | AoneError::BinaryFile => (AiProjectAgentErrorCode::ProviderFailed, true),
    };
    AiProjectAgentResponse::Error {
        code,
        message,
        retryable,
    }
}

#[cfg(test)]
pub(super) fn project_agent_error_response_for_test(error: AoneError) -> AiProjectAgentResponse {
    project_agent_error_response(error)
}

#[tauri::command]
pub async fn ai_explain_project_environment(
    request: AiExplainProjectEnvironmentRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    environments: State<'_, ProjectEnvironmentState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiExplanation> {
    let _reservation = secrets.reserve_ai_call()?;
    let current = workspaces.workspace()?;
    if current.id != request.workspace_id {
        return Err(AoneError::InvalidRequest(
            "project environment report belongs to another workspace".into(),
        ));
    }
    let report = environments.latest_report(&request.workspace_id, &request.report_id)?;
    let provider = secrets.provider_configuration()?;
    let evidence = build_project_environment_evidence(&report, &runtime)?;
    confirm_project_environment_call(
        &app,
        &provider,
        evidence.references.len(),
        evidence.input.len(),
    )
    .await?;
    workspaces.while_workspace_current(&request.workspace_id, || Ok(()))?;

    let request_body =
        project_environment_provider_request(provider.kind, &provider.model, &evidence.input);
    let redaction_values = runtime.secret_values_for_redaction();
    let answer =
        request_provider(&provider, &request_body, &redaction_values, &current.root).await?;
    let answer = format!(
        "AI inference over the deterministic project setup report:\n\n{}",
        answer.trim()
    );

    workspaces.while_workspace_current(&request.workspace_id, || {
        runtime.with_workspace_current(&current.root, || {
            let request_id = format!("ai:project-environment:{}", Uuid::now_v7());
            let metadata = std::collections::BTreeMap::from([
                ("provider".into(), json!(provider.kind.id())),
                ("model".into(), json!(&provider.model)),
                ("reportId".into(), json!(report.report_id)),
                ("evidenceCount".into(), json!(evidence.references.len())),
            ]);
            let event = runtime.next_event(
                None,
                "ai.projectEnvironmentExplanation",
                "AI-generated project setup inference",
                EvidenceKind::Inferred,
                Some(request_id),
                metadata,
            );
            publish_event(&app, &runtime, event);

            AiExplanation {
                answer,
                evidence: evidence.references,
                model: provider.model,
            }
        })
    })
}
