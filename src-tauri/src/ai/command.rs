use serde_json::json;
use tauri::{AppHandle, State};
use uuid::Uuid;

use super::{
    evidence::build_evidence,
    provider::{confirm_provider_call, provider_request, request_provider},
    state::SecretState,
};
use crate::{
    domain::{AiExplainRequest, AiExplanation, EvidenceKind},
    error::AoneResult,
    runner::{RuntimeState, publish_event},
    state::AppState,
};

#[tauri::command]
pub async fn ai_explain(
    request: AiExplainRequest,
    app: AppHandle,
    workspaces: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiExplanation> {
    // Reserve before opening a native dialog so renderer races cannot create
    // multiple simultaneous provider requests or overlapping consent prompts.
    let _reservation = secrets.reserve_ai_call()?;
    let workspace_root = workspaces.workspace()?.root;
    let (provider, redaction_values, evidence) =
        workspaces.while_workspace_current(&request.workspace_id, || {
            let provider = secrets.provider_configuration()?;
            let redaction_values = runtime.secret_values_for_redaction();
            let evidence = build_evidence(&request, &secrets, &redaction_values)?;
            Ok((provider, redaction_values, evidence))
        })?;
    confirm_provider_call(
        &app,
        &provider,
        evidence.references.len(),
        evidence.input.len(),
    )
    .await?;
    workspaces.while_workspace_current(&request.workspace_id, || Ok(()))?;

    let request_body = provider_request(provider.kind, &provider.model, &evidence.input);
    let answer =
        request_provider(&provider, &request_body, &redaction_values, &workspace_root).await?;
    let answer = format!(
        "AI inference (not an observed runtime fact):\n\n{}",
        answer.trim()
    );

    workspaces.while_workspace_current(&request.workspace_id, || {
        let request_id = format!("ai:{}", Uuid::now_v7());
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("provider".into(), json!(provider.kind.id()));
        metadata.insert("model".into(), json!(&provider.model));
        metadata.insert("evidenceCount".into(), json!(evidence.references.len()));
        metadata.insert(
            "evidenceIds".into(),
            json!(
                evidence
                    .references
                    .iter()
                    .map(|reference| &reference.id)
                    .collect::<Vec<_>>()
            ),
        );
        let event = runtime.next_event(
            None,
            "ai.explanation",
            "AI-generated inference",
            EvidenceKind::Inferred,
            Some(request_id),
            metadata,
        );
        publish_event(&app, &runtime, event);

        Ok(AiExplanation {
            answer,
            evidence: evidence.references,
            model: provider.model.clone(),
        })
    })
}
