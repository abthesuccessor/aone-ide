use std::{
    collections::BTreeMap,
    path::{Component, Path},
    sync::Arc,
};

use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::super::{
    RuntimeState, is_sensitive_name, redact_text, session::RunStreamSession,
    timestamp::utc_timestamp,
};
use crate::{
    domain::{
        DebugBranchOutcome, DebugDataField, DebugDataPreview, DebugEventKind, DebugFlowStage,
        DebugSafePointState, DebugSourceLocation, DebugWorkflowEvent, EvidenceKind, RuntimeEvent,
    },
    scanner::is_hard_denied,
    state::AppState,
};

use super::registry::MAX_DEBUG_EVENTS_PER_SESSION;

#[cfg(test)]
use super::{
    control::{QueuedDebugControl, debug_control_channel, encode_control_line},
    registry::DebugRegistry,
};

pub(in crate::runner) const DEBUG_PREFIX: &str = "AONE_DEBUG_V1 ";
const MAX_ID_CHARS: usize = 128;
const MAX_SOURCE_PATH_CHARS: usize = 1_024;
const MAX_SOURCE_LINE: usize = 10_000_000;
const MAX_PREVIEW_FIELDS: usize = 32;
const MAX_PREVIEW_COUNT: u64 = 1_000_000_000;
const MAX_SEMANTIC_CHARS: usize = 128;
const MAX_DERIVED_LABEL_CHARS: usize = 240;

#[derive(Clone)]
pub(in crate::runner) struct DebugSessionContext {
    pub(in crate::runner) nonce: Arc<Zeroizing<String>>,
    pub(in crate::runner) workspace_id: String,
    pub(in crate::runner) debug_session_id: String,
}

pub(in crate::runner) enum DebugLine {
    NotDebug,
    Accepted(RuntimeEvent),
    Rejected(&'static str),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DebugEnvelope {
    nonce: String,
    debug_session_id: String,
    workflow_id: String,
    sequence: u64,
    control_epoch: u64,
    step_id: String,
    #[serde(default)]
    parent_step_id: Option<String>,
    kind: DebugEventKind,
    flow_stage: DebugFlowStage,
    source: DebugSource,
    safe_point_state: DebugSafePointState,
    #[serde(default)]
    branch_outcome: Option<DebugBranchOutcome>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    data_preview: Option<DebugDataPreview>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DebugSource {
    relative_path: String,
    line: usize,
    #[serde(default)]
    line_end: Option<usize>,
    #[serde(default)]
    node_id: Option<String>,
}

pub(in crate::runner) fn ingest_debug_line(
    app: &AppHandle,
    runtime: &RuntimeState,
    debug_session: &DebugSessionContext,
    run_session: &RunStreamSession,
    line: &str,
    secret_values: &[Zeroizing<String>],
) -> DebugLine {
    let Some(payload) = line.strip_prefix(DEBUG_PREFIX) else {
        return DebugLine::NotDebug;
    };
    let Ok(mut envelope) = serde_json::from_str::<DebugEnvelope>(payload) else {
        return DebugLine::Rejected("invalid bounded debug JSON object");
    };
    if envelope.nonce != debug_session.nonce.as_str()
        || envelope.debug_session_id != debug_session.debug_session_id
    {
        return DebugLine::Rejected("debug nonce or session identity mismatch");
    }
    if !valid_id(&envelope.workflow_id)
        || !valid_id(&envelope.step_id)
        || envelope
            .parent_step_id
            .as_deref()
            .is_some_and(|id| !valid_id(id))
        || envelope.parent_step_id.as_deref() == Some(envelope.step_id.as_str())
    {
        return DebugLine::Rejected("invalid debug workflow, step, or parent identity");
    }
    if envelope.sequence == 0 || envelope.sequence > MAX_DEBUG_EVENTS_PER_SESSION as u64 {
        return DebugLine::Rejected("debug safe-point sequence exceeds the session bound");
    }
    if !valid_source(&envelope.source) {
        return DebugLine::Rejected("invalid workspace-relative debug source location");
    }
    if (envelope.kind == DebugEventKind::Branch) != envelope.branch_outcome.is_some() {
        return DebugLine::Rejected("branch outcome is required only for branch safe points");
    }
    if envelope
        .operation
        .as_deref()
        .is_some_and(|value| !valid_semantic(value))
        || envelope
            .resource
            .as_deref()
            .is_some_and(|value| !valid_semantic(value))
        || requires_operation(envelope.kind) && envelope.operation.is_none()
    {
        return DebugLine::Rejected(
            "debug operation or resource is not a bounded semantic identifier",
        );
    }
    if let Some(preview) = envelope.data_preview.as_mut()
        && sanitize_preview(preview).is_err()
    {
        return DebugLine::Rejected("debug data preview is not shape-only or exceeds its bounds");
    }
    if envelope_contains_known_secret(&envelope, secret_values) {
        return DebugLine::Rejected(
            "debug protocol identifiers or schema metadata contain a loaded secret value",
        );
    }

    let mapped_node_id =
        exact_source_node(app, runtime, debug_session, run_session, &envelope.source);
    let label = safe_label(&envelope);
    let workflow_event = DebugWorkflowEvent {
        id: format!("debug:{}", Uuid::now_v7()),
        workflow_id: envelope.workflow_id,
        sequence: envelope.sequence,
        control_epoch: envelope.control_epoch,
        step_id: envelope.step_id,
        parent_step_id: envelope.parent_step_id,
        kind: envelope.kind,
        label: redact_text(&label, secret_values),
        flow_stage: envelope.flow_stage,
        source: DebugSourceLocation {
            relative_path: redact_text(&envelope.source.relative_path, secret_values),
            line: envelope.source.line,
            line_end: envelope.source.line_end,
        },
        safe_point_state: envelope.safe_point_state,
        branch_outcome: envelope.branch_outcome,
        operation: envelope
            .operation
            .map(|value| redact_text(&value, secret_values)),
        resource: envelope
            .resource
            .map(|value| redact_text(&value, secret_values)),
        data_preview: envelope.data_preview,
        timestamp: utc_timestamp(),
    };
    if runtime
        .accept_debug_event(run_session, workflow_event.clone())
        .is_err()
    {
        return DebugLine::Rejected(
            "debug sequence, workflow, parent, state, or control epoch was rejected",
        );
    }

    let mut metadata = BTreeMap::<String, Value>::from([
        ("debugProtocol".into(), json!("AONE_DEBUG_V1")),
        (
            "debugSessionId".into(),
            json!(debug_session.debug_session_id),
        ),
        ("workflowId".into(), json!(workflow_event.workflow_id)),
        ("debugWorkflowEventId".into(), json!(workflow_event.id)),
        ("sequence".into(), json!(workflow_event.sequence)),
        ("controlEpoch".into(), json!(workflow_event.control_epoch)),
        ("stepId".into(), json!(workflow_event.step_id)),
        ("flowStage".into(), json!(workflow_event.flow_stage)),
        (
            "safePointState".into(),
            json!(workflow_event.safe_point_state),
        ),
        (
            "sourceRelativePath".into(),
            json!(workflow_event.source.relative_path),
        ),
        ("sourceLine".into(), json!(workflow_event.source.line)),
        ("processReported".into(), json!(true)),
        ("rawValuesAccepted".into(), json!(false)),
        ("schemaNamesProcessReported".into(), json!(true)),
        ("reportedNamesValueSafetyUnverified".into(), json!(true)),
        (
            "sourceMapping".into(),
            json!(if mapped_node_id.is_some() {
                "candidate"
            } else {
                "unmapped"
            }),
        ),
        ("mappingKind".into(), json!("processReportedSourceRange")),
    ]);
    if let Some(line_end) = workflow_event.source.line_end {
        metadata.insert("sourceLineEnd".into(), json!(line_end));
    }
    if let Some(parent) = workflow_event.parent_step_id.as_ref() {
        metadata.insert("parentStepId".into(), json!(parent));
    }
    if let Some(outcome) = workflow_event.branch_outcome {
        metadata.insert("branchOutcome".into(), json!(outcome));
    }
    if let Some(operation) = workflow_event.operation.as_ref() {
        metadata.insert("operation".into(), json!(operation));
    }
    if let Some(resource) = workflow_event.resource.as_ref() {
        metadata.insert("resource".into(), json!(resource));
    }
    if let Some(preview) = workflow_event.data_preview.as_ref() {
        metadata.insert("dataPreview".into(), json!(preview));
    }
    if let Some(mapped_node_id) = mapped_node_id {
        metadata.insert("mappedNodeId".into(), json!(mapped_node_id));
    }
    DebugLine::Accepted(runtime.next_event(
        Some(run_session.run_id().to_owned()),
        format!("debug.{}", debug_kind_name(workflow_event.kind)),
        workflow_event.label,
        EvidenceKind::Observed,
        Some(debug_session.debug_session_id.clone()),
        metadata,
    ))
}

fn safe_label(envelope: &DebugEnvelope) -> String {
    let label = match envelope.kind {
        DebugEventKind::Request => envelope.resource.as_ref().map_or_else(
            || "Request safe point".into(),
            |value| format!("Request {value}"),
        ),
        DebugEventKind::Method => format!(
            "Method {}",
            envelope.operation.as_deref().unwrap_or("unknown")
        ),
        DebugEventKind::Line => format!("Line {}", envelope.source.line),
        DebugEventKind::Branch => format!(
            "Branch {}",
            envelope
                .branch_outcome
                .map(branch_outcome_name)
                .unwrap_or("unknown")
        ),
        DebugEventKind::Database => format!(
            "Database {} {}",
            envelope.operation.as_deref().unwrap_or("operation"),
            envelope.resource.as_deref().unwrap_or("resource")
        ),
        DebugEventKind::Agent => format!(
            "Agent {}",
            envelope.operation.as_deref().unwrap_or("operation")
        ),
        DebugEventKind::External => format!(
            "External {}",
            envelope.operation.as_deref().unwrap_or("call")
        ),
        DebugEventKind::Response => "Response safe point".into(),
    };
    label.chars().take(MAX_DERIVED_LABEL_CHARS).collect()
}

fn exact_source_node(
    app: &AppHandle,
    runtime: &RuntimeState,
    debug_session: &DebugSessionContext,
    run_session: &RunStreamSession,
    source: &DebugSource,
) -> Option<String> {
    let state = app.state::<AppState>();
    let context = state.workspace().ok()?;
    if context.id != debug_session.workspace_id {
        return None;
    }
    state
        .while_workspace_current(&debug_session.workspace_id, || {
            if !runtime.run_session_is_current(run_session) {
                return Ok(None);
            }
            context.store.lock().resolve_trace_source(
                source.node_id.as_deref(),
                &source.relative_path,
                source.line,
            )
        })
        .ok()
        .flatten()
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_ID_CHARS
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        })
}

fn valid_source(source: &DebugSource) -> bool {
    if source.relative_path.is_empty()
        || source.relative_path.chars().count() > MAX_SOURCE_PATH_CHARS
        || source.relative_path.chars().any(char::is_control)
        || source.line == 0
        || source.line > MAX_SOURCE_LINE
        || source
            .line_end
            .is_some_and(|end| end < source.line || end > MAX_SOURCE_LINE)
        || source.node_id.as_deref().is_some_and(|id| !valid_id(id))
    {
        return false;
    }
    let path = Path::new(&source.relative_path);
    !path.is_absolute()
        && !is_hard_denied(path)
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_semantic(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_SEMANTIC_CHARS
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == ' '
                || matches!(character, '_' | '-' | '.' | ':' | '/')
        })
}

fn sanitize_preview(preview: &mut DebugDataPreview) -> Result<(), ()> {
    if preview.fields.len() > MAX_PREVIEW_FIELDS
        || [preview.item_count, preview.row_count, preview.null_count]
            .into_iter()
            .flatten()
            .any(|count| count > MAX_PREVIEW_COUNT)
        || preview
            .type_name
            .as_deref()
            .is_some_and(|value| !valid_semantic(value))
    {
        return Err(());
    }
    for DebugDataField {
        name, value_type, ..
    } in &mut preview.fields
    {
        if !valid_semantic(name) || !valid_semantic(value_type) {
            return Err(());
        }
        if is_sensitive_name(name) {
            *name = "[sensitive-field]".into();
        }
    }
    Ok(())
}

fn envelope_contains_known_secret(
    envelope: &DebugEnvelope,
    secret_values: &[Zeroizing<String>],
) -> bool {
    let mut values = vec![
        envelope.workflow_id.as_str(),
        envelope.step_id.as_str(),
        envelope.source.relative_path.as_str(),
    ];
    values.extend(envelope.parent_step_id.as_deref());
    values.extend(envelope.source.node_id.as_deref());
    values.extend(envelope.operation.as_deref());
    values.extend(envelope.resource.as_deref());
    if let Some(preview) = envelope.data_preview.as_ref() {
        values.extend(preview.type_name.as_deref());
        for field in &preview.fields {
            values.push(&field.name);
            values.push(&field.value_type);
        }
    }
    values.into_iter().any(|value| {
        secret_values
            .iter()
            .any(|secret| !secret.is_empty() && value.contains(secret.as_str()))
    })
}

fn requires_operation(kind: DebugEventKind) -> bool {
    matches!(
        kind,
        DebugEventKind::Method
            | DebugEventKind::Database
            | DebugEventKind::Agent
            | DebugEventKind::External
    )
}

fn debug_kind_name(kind: DebugEventKind) -> &'static str {
    match kind {
        DebugEventKind::Request => "request",
        DebugEventKind::Method => "method",
        DebugEventKind::Line => "line",
        DebugEventKind::Branch => "branch",
        DebugEventKind::Database => "database",
        DebugEventKind::Agent => "agent",
        DebugEventKind::External => "external",
        DebugEventKind::Response => "response",
    }
}

fn branch_outcome_name(outcome: DebugBranchOutcome) -> &'static str {
    match outcome {
        DebugBranchOutcome::Then => "then",
        DebugBranchOutcome::Else => "else",
        DebugBranchOutcome::Case => "case",
        DebugBranchOutcome::Loop => "loop",
        DebugBranchOutcome::ShortCircuit => "short circuit",
        DebugBranchOutcome::Unknown => "unknown",
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
