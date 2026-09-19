use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::wire::{DecodedSpan, ExportTraceServiceRequest, SpanCategory};
use crate::{
    domain::{EvidenceKind, RuntimeEvent},
    error::{AoneError, AoneResult},
    runner::{RuntimeState, publish_event, redact_text},
    scanner::is_hard_denied,
    state::{AppState, WorkspaceContext},
};

pub(super) const OTLP_TRACE_PROTOCOL: &str = "OTLP_HTTP_JSON";

pub(super) struct IngestResult {
    pub(super) accepted: usize,
    pub(super) rejected: usize,
}

pub(super) fn ingest_export(
    app: &AppHandle,
    receiver_id: &str,
    workspace_id: &str,
    export: ExportTraceServiceRequest,
) -> AoneResult<IngestResult> {
    let app_state = app.state::<AppState>();
    let runtime = app.state::<RuntimeState>();
    let workspace = app_state.workspace()?;
    if workspace.id != workspace_id {
        return Err(AoneError::InvalidRequest(
            "the OTLP receiver belongs to a different workspace".into(),
        ));
    }
    let mut decoded = export.decode();
    if decoded.rejected > 0 {
        runtime
            .otlp
            .lock()
            .record_rejected(receiver_id, decoded.rejected);
    }
    let mut accepted = 0_usize;
    let mut rejected = decoded.rejected;
    let secret_values = runtime.secret_values_for_redaction();
    for span in decoded.spans.drain(..) {
        if runtime
            .otlp
            .lock()
            .reserve_span(receiver_id, &span.trace_id, &span.span_id)
            .is_err()
        {
            rejected = rejected.saturating_add(1);
            continue;
        }
        let event = runtime_event(&workspace, &runtime, receiver_id, span, &secret_values);
        app_state.while_workspace_current(workspace_id, || {
            runtime.with_workspace_current(&workspace.root, || {
                publish_event(app, &runtime, event);
            })
        })?;
        accepted = accepted.saturating_add(1);
    }
    Ok(IngestResult { accepted, rejected })
}

fn runtime_event(
    workspace: &WorkspaceContext,
    runtime: &RuntimeState,
    receiver_id: &str,
    span: DecodedSpan,
    secret_values: &[zeroize::Zeroizing<String>],
) -> RuntimeEvent {
    let source_path = span
        .source_path
        .as_deref()
        .and_then(|path| safe_workspace_source(&workspace.root, path));
    let mapped_node_id = source_path
        .as_deref()
        .zip(span.source_line)
        .and_then(|(path, line)| {
            workspace
                .store
                .lock()
                .resolve_trace_source(None, path, line)
                .ok()
                .flatten()
        });
    let mut metadata = BTreeMap::<String, Value>::from([
        ("traceProtocol".into(), json!(OTLP_TRACE_PROTOCOL)),
        ("spanId".into(), json!(span.span_id)),
        ("phase".into(), json!("event")),
        (
            "flowStage".into(),
            json!(
                span.flow_stage
                    .unwrap_or_else(|| span.category.flow_stage())
            ),
        ),
        ("durationMs".into(), json!(span.duration_ms)),
        ("status".into(), json!(span.status)),
        ("errorStatus".into(), json!(span.status == "error")),
        ("processReported".into(), json!(true)),
        ("attributePolicy".into(), json!("semanticAllowlistOnly")),
        ("rawRuntimeValuesRetained".into(), json!(false)),
        ("dataOperation".into(), json!(data_operation(&span))),
    ]);
    insert_redacted(
        &mut metadata,
        "serviceName",
        span.service_name.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "serviceNamespace",
        span.service_namespace.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "codeFunctionName",
        span.function_name.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "httpMethod",
        span.http_method.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "httpRoute",
        span.http_route.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "dbSystem",
        span.db_system.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "dbOperation",
        span.db_operation.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "dbCollection",
        span.db_collection.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "messagingSystem",
        span.messaging_system.as_deref(),
        secret_values,
    );
    insert_redacted(
        &mut metadata,
        "messagingOperation",
        span.messaging_operation.as_deref(),
        secret_values,
    );
    if let Some(parent_span_id) = &span.parent_span_id {
        metadata.insert("parentSpanId".into(), json!(parent_span_id));
    }
    if let Some(path) = source_path {
        metadata.insert("sourceRelativePath".into(), json!(path));
        if let Some(line) = span.source_line {
            metadata.insert("sourceLine".into(), json!(line));
        }
        metadata.insert("mappingKind".into(), json!("processReportedSourceRange"));
        metadata.insert(
            "sourceMapping".into(),
            json!(if mapped_node_id.is_some() {
                "candidate"
            } else {
                "unmapped"
            }),
        );
    } else {
        metadata.insert("sourceMapping".into(), json!("unmapped"));
    }
    if let Some(mapped_node_id) = mapped_node_id {
        metadata.insert("mappedNodeId".into(), json!(mapped_node_id));
    }
    let label = display_label(&span);
    let mut event = runtime.next_event(
        Some(format!("otlp:{receiver_id}")),
        span.category.event_kind(),
        redact_text(&label, secret_values),
        EvidenceKind::Observed,
        Some(span.trace_id),
        metadata,
    );
    event.timestamp = timestamp_from_unix_nanos(span.start_unix_nanos);
    event
}

fn display_label(span: &DecodedSpan) -> String {
    match span.category {
        SpanCategory::HttpServer | SpanCategory::HttpClient => {
            match (span.http_method.as_deref(), span.http_route.as_deref()) {
                (Some(method), Some(route)) => format!("{method} {route}"),
                (Some(method), None) => format!("{method} HTTP request"),
                _ => "HTTP request".into(),
            }
        }
        SpanCategory::Database => {
            let operation = span.db_operation.as_deref().unwrap_or("database operation");
            match span.db_collection.as_deref() {
                Some(collection) => format!("{operation} {collection}"),
                None => operation.to_owned(),
            }
        }
        SpanCategory::Event => match (
            span.messaging_operation.as_deref(),
            span.messaging_system.as_deref(),
        ) {
            (Some(operation), Some(system)) => format!("{operation} via {system}"),
            (Some(operation), None) => operation.to_owned(),
            _ => "Messaging operation".into(),
        },
        SpanCategory::External => "External call".into(),
        SpanCategory::Function => span
            .function_name
            .clone()
            .unwrap_or_else(|| "Function span".into()),
    }
}

fn insert_redacted(
    metadata: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<&str>,
    secret_values: &[zeroize::Zeroizing<String>],
) {
    if let Some(value) = value {
        metadata.insert(key.into(), json!(redact_text(value, secret_values)));
    }
}

fn data_operation(span: &DecodedSpan) -> &'static str {
    match span.category {
        SpanCategory::HttpServer => "receivingInput",
        SpanCategory::HttpClient | SpanCategory::External => "callingExternalService",
        SpanCategory::Event
            if matches!(
                span.messaging_operation.as_deref(),
                Some("publish" | "send")
            ) =>
        {
            "publishingEvent"
        }
        SpanCategory::Event => "handlingEvent",
        SpanCategory::Database => {
            let operation = span.db_operation.as_deref().unwrap_or_default();
            if ["select", "get", "read", "find", "query"]
                .iter()
                .any(|candidate| operation.eq_ignore_ascii_case(candidate))
            {
                "readingDatabase"
            } else if ["insert", "update", "delete", "create", "write", "upsert"]
                .iter()
                .any(|candidate| operation.eq_ignore_ascii_case(candidate))
            {
                "writingDatabase"
            } else {
                "databaseOperation"
            }
        }
        SpanCategory::Function => "executingFunction",
    }
}

fn safe_workspace_source(root: &Path, reported: &str) -> Option<String> {
    let normalized = reported.replace('\\', "/");
    let reported_path = Path::new(&normalized);
    let relative = if reported_path.is_absolute() {
        reported_path.strip_prefix(root).ok()?
    } else {
        reported_path
    };
    if is_hard_denied(relative)
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let parts = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn timestamp_from_unix_nanos(nanos: u64) -> String {
    let seconds = i64::try_from(nanos / 1_000_000_000).unwrap_or(i64::MAX);
    let millis = (nanos % 1_000_000_000) / 1_000_000;
    let days = seconds.div_euclid(86_400);
    let seconds_in_day = seconds.rem_euclid(86_400);
    let hour = seconds_in_day / 3_600;
    let minute = (seconds_in_day % 3_600) / 60;
    let second = seconds_in_day % 60;
    let (year, month, day) = crate::runner::timestamp::civil_date_from_unix_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_paths_are_workspace_relative_and_sensitive_paths_are_rejected() {
        let root = Path::new("/workspace");
        assert_eq!(
            safe_workspace_source(root, "/workspace/src/service.rs").as_deref(),
            Some("src/service.rs")
        );
        assert_eq!(
            safe_workspace_source(root, "src\\service.rs").as_deref(),
            Some("src/service.rs")
        );
        assert!(safe_workspace_source(root, "/other/service.rs").is_none());
        assert!(safe_workspace_source(root, "../service.rs").is_none());
        assert!(safe_workspace_source(root, ".env").is_none());
    }

    #[test]
    fn timestamp_uses_span_start_time_without_local_timezone() {
        assert_eq!(
            timestamp_from_unix_nanos(1_544_712_660_123_000_000),
            "2018-12-13T14:51:00.123Z"
        );
    }
}
