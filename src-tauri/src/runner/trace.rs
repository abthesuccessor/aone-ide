use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

use super::{RuntimeState, redact_text, session::RunStreamSession};
use crate::{
    domain::{EvidenceKind, RuntimeEvent},
    scanner::is_hard_denied,
    state::AppState,
};

pub(super) const TRACE_PREFIX: &str = "AONE_TRACE_V1 ";
const MAX_ID_CHARS: usize = 128;
const MAX_LABEL_CHARS: usize = 240;
const MAX_SOURCE_PATH_CHARS: usize = 1_024;
const MAX_SOURCE_LINE: usize = 10_000_000;
const MAX_DURATION_MS: u64 = 86_400_000;

#[derive(Clone)]
pub(super) struct TraceSession {
    pub(super) nonce: Zeroizing<String>,
    pub(super) workspace_id: String,
}

pub(super) enum TraceLine {
    NotTrace,
    Accepted(RuntimeEvent),
    Rejected(&'static str),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TraceEnvelope {
    nonce: String,
    trace_id: String,
    span_id: String,
    #[serde(default)]
    parent_span_id: Option<String>,
    kind: String,
    phase: String,
    label: String,
    source: TraceSource,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TraceSource {
    relative_path: String,
    line: usize,
    #[serde(default)]
    node_id: Option<String>,
}

pub(super) fn ingest_trace_line(
    app: &AppHandle,
    runtime: &RuntimeState,
    trace_session: &TraceSession,
    run_session: &RunStreamSession,
    line: &str,
    secret_values: &[Zeroizing<String>],
) -> TraceLine {
    let Some(payload) = line.strip_prefix(TRACE_PREFIX) else {
        return TraceLine::NotTrace;
    };
    let Ok(envelope) = serde_json::from_str::<TraceEnvelope>(payload) else {
        return TraceLine::Rejected("invalid bounded JSON object");
    };
    if envelope.nonce != trace_session.nonce.as_str() {
        return TraceLine::Rejected("run nonce mismatch");
    }
    if !valid_id(&envelope.trace_id)
        || !valid_id(&envelope.span_id)
        || envelope
            .parent_span_id
            .as_deref()
            .is_some_and(|id| !valid_id(id))
        || envelope.parent_span_id.as_deref() == Some(envelope.span_id.as_str())
    {
        return TraceLine::Rejected("invalid trace, span, or parent identity");
    }
    if !matches!(
        envelope.kind.as_str(),
        "http.server"
            | "http.client"
            | "function"
            | "database"
            | "external"
            | "agent"
            | "event"
            | "job"
    ) {
        return TraceLine::Rejected("unsupported trace kind");
    }
    if !matches!(envelope.phase.as_str(), "start" | "end" | "event") {
        return TraceLine::Rejected("unsupported trace phase");
    }
    if envelope.label.is_empty()
        || envelope.label.chars().count() > MAX_LABEL_CHARS
        || envelope.label.chars().any(char::is_control)
    {
        return TraceLine::Rejected("invalid trace label");
    }
    if envelope
        .duration_ms
        .is_some_and(|value| value > MAX_DURATION_MS)
    {
        return TraceLine::Rejected("trace duration exceeds the bound");
    }
    if envelope
        .status
        .as_deref()
        .is_some_and(|status| !matches!(status, "ok" | "error" | "cancelled" | "unset"))
    {
        return TraceLine::Rejected("unsupported trace status");
    }
    if !valid_source(&envelope.source) {
        return TraceLine::Rejected("invalid workspace-relative source location");
    }

    let mapped_node_id =
        exact_source_node(app, runtime, trace_session, run_session, &envelope.source);
    if runtime
        .accept_trace_span(
            run_session,
            &envelope.trace_id,
            &envelope.span_id,
            envelope.parent_span_id.as_deref(),
            &envelope.phase,
        )
        .is_err()
    {
        return TraceLine::Rejected("trace parent or phase sequence was rejected");
    }

    let mut metadata = BTreeMap::<String, Value>::from([
        ("traceProtocol".into(), json!("AONE_TRACE_V1")),
        ("spanId".into(), json!(envelope.span_id)),
        ("phase".into(), json!(envelope.phase)),
        (
            "sourceRelativePath".into(),
            json!(redact_text(&envelope.source.relative_path, secret_values)),
        ),
        ("sourceLine".into(), json!(envelope.source.line)),
        (
            "sourceMapping".into(),
            json!(if mapped_node_id.is_some() {
                "candidate"
            } else {
                "unmapped"
            }),
        ),
        ("mappingKind".into(), json!("processReportedSourceRange")),
        ("processReported".into(), json!(true)),
    ]);
    if let Some(parent) = envelope.parent_span_id {
        metadata.insert("parentSpanId".into(), json!(parent));
    }
    if let Some(duration) = envelope.duration_ms {
        metadata.insert("durationMs".into(), json!(duration));
    }
    if let Some(status) = envelope.status {
        metadata.insert("status".into(), json!(status));
    }
    if let Some(mapped_node_id) = mapped_node_id {
        metadata.insert("mappedNodeId".into(), json!(mapped_node_id));
    }
    let event = runtime.next_event(
        Some(run_session.run_id().to_owned()),
        format!(
            "trace.{}.{}",
            envelope.kind,
            metadata["phase"].as_str().unwrap_or("event")
        ),
        redact_text(&envelope.label, secret_values),
        EvidenceKind::Observed,
        Some(envelope.trace_id),
        metadata,
    );
    TraceLine::Accepted(event)
}

fn exact_source_node(
    app: &AppHandle,
    runtime: &RuntimeState,
    trace_session: &TraceSession,
    run_session: &RunStreamSession,
    source: &TraceSource,
) -> Option<String> {
    let state = app.state::<AppState>();
    let context = state.workspace().ok()?;
    if context.id != trace_session.workspace_id {
        return None;
    }
    state
        .while_workspace_current(&trace_session.workspace_id, || {
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

fn valid_source(source: &TraceSource) -> bool {
    if source.relative_path.is_empty()
        || source.relative_path.chars().count() > MAX_SOURCE_PATH_CHARS
        || source.relative_path.chars().any(char::is_control)
        || source.line == 0
        || source.line > MAX_SOURCE_LINE
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

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str) -> TraceSource {
        TraceSource {
            relative_path: path.into(),
            line: 10,
            node_id: None,
        }
    }

    #[test]
    fn trace_protocol_rejects_sensitive_or_non_relative_source_paths() {
        for path in [
            ".env",
            ".aws/config",
            ".cargo/credentials.toml",
            "infra/terraform.tfstate",
            "../outside.rs",
            "/absolute/source.rs",
        ] {
            assert!(!valid_source(&source(path)), "accepted {path}");
        }
        assert!(valid_source(&source("backend/src/service.rs")));
    }

    #[test]
    fn trace_protocol_rejects_unknown_fields_and_invalid_ids() {
        let json = serde_json::json!({
            "nonce": "nonce",
            "traceId": "trace-1",
            "spanId": "span-1",
            "kind": "function",
            "phase": "event",
            "label": "work",
            "source": { "relativePath": "src/work.rs", "line": 1 },
            "unexpected": true
        });
        assert!(serde_json::from_value::<TraceEnvelope>(json).is_err());
        assert!(valid_id("trace:one-2_ok"));
        assert!(!valid_id("trace/one"));
        assert!(!valid_id(&"x".repeat(MAX_ID_CHARS + 1)));
    }
}
