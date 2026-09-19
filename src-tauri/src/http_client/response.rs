use std::collections::BTreeMap;

use reqwest::{Url, header::HeaderMap};
use serde_json::{Value, json};
use tauri::AppHandle;
use zeroize::Zeroizing;

use super::request::PreparedRequest;
use crate::{
    domain::{ApiHeader, ApiResponse, EvidenceKind},
    error::{AoneError, AoneResult},
    runner::{RuntimeState, is_sensitive_name, publish_event, redact_json, redact_text},
};

const MAX_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;

pub(super) async fn read_response_bounded(
    mut response: reqwest::Response,
) -> AoneResult<(Vec<u8>, bool)> {
    let mut body = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AoneError::Task(format!("failed to read HTTP response: {error}")))?
    {
        let remaining = MAX_RESPONSE_BODY_BYTES.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
        if body.len() == MAX_RESPONSE_BODY_BYTES {
            truncated = true;
            break;
        }
    }
    Ok((body, truncated))
}

pub(super) fn emit_request_started(
    app: &AppHandle,
    runtime: &RuntimeState,
    request_id: &str,
    request: &PreparedRequest,
) {
    let metadata = request_event_metadata(request);
    let event = runtime.next_event(
        None,
        "http.request",
        format!("{} request", request.method.as_str()),
        EvidenceKind::Observed,
        Some(request.trace_context.as_ref().map_or_else(
            || request_id.to_string(),
            |context| context.trace_id.clone(),
        )),
        metadata,
    );
    publish_event(app, runtime, event);
}

pub(super) fn request_event_metadata(request: &PreparedRequest) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("method".into(), json!(request.method.as_str()));
    metadata.insert("url".into(), json!(redact_url(&request.url)));
    metadata.insert("headerNames".into(), json!(request_header_names(request)));
    if let Some(body) = &request.body {
        metadata.insert("bodyBytes".into(), json!(body.len()));
    }
    metadata.insert("timeoutMs".into(), json!(request.timeout_ms));
    if request.trace_context.is_some() {
        metadata.insert("traceContextInjected".into(), json!(true));
    }
    metadata
}

pub(super) fn emit_request_completed(
    app: &AppHandle,
    runtime: &RuntimeState,
    request: &PreparedRequest,
    response: &ApiResponse,
) {
    let metadata = response_event_metadata(request, response);
    let event = runtime.next_event(
        None,
        "http.response",
        format!("HTTP {} in {} ms", response.status, response.duration_ms),
        EvidenceKind::Observed,
        Some(request.trace_context.as_ref().map_or_else(
            || response.request_id.clone(),
            |context| context.trace_id.clone(),
        )),
        metadata,
    );
    publish_event(app, runtime, event);
    emit_api_client_span(
        app,
        runtime,
        request,
        response.duration_ms,
        response.status >= 400,
    );
}

pub(super) fn response_event_metadata(
    request: &PreparedRequest,
    response: &ApiResponse,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("method".into(), json!(request.method.as_str()));
    metadata.insert("url".into(), json!(redact_url(&request.url)));
    metadata.insert("status".into(), json!(response.status));
    metadata.insert("durationMs".into(), json!(response.duration_ms));
    metadata.insert(
        "headerNames".into(),
        json!(
            response
                .headers
                .iter()
                .map(|header| header.name.clone())
                .collect::<Vec<_>>()
        ),
    );
    metadata.insert("bodyBytes".into(), json!(response.body.len()));
    metadata.insert("bodyTruncated".into(), json!(response.truncated));
    metadata
}

pub(super) fn emit_request_failed(
    app: &AppHandle,
    runtime: &RuntimeState,
    request_id: &str,
    request: &PreparedRequest,
    duration_ms: u64,
    message: &str,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("method".into(), json!(request.method.as_str()));
    metadata.insert("url".into(), json!(redact_url(&request.url)));
    metadata.insert("durationMs".into(), json!(duration_ms));
    metadata.insert("error".into(), json!(message));
    let event = runtime.next_event(
        None,
        "http.failed",
        "HTTP request failed",
        EvidenceKind::Observed,
        Some(request.trace_context.as_ref().map_or_else(
            || request_id.to_string(),
            |context| context.trace_id.clone(),
        )),
        metadata,
    );
    publish_event(app, runtime, event);
    emit_api_client_span(app, runtime, request, duration_ms, true);
}

fn emit_api_client_span(
    app: &AppHandle,
    runtime: &RuntimeState,
    request: &PreparedRequest,
    duration_ms: u64,
    failed: bool,
) {
    let Some(context) = request.trace_context.as_ref() else {
        return;
    };
    let metadata = BTreeMap::from([
        ("traceProtocol".into(), json!("W3C_TRACE_CONTEXT")),
        ("spanId".into(), json!(context.span_id.clone())),
        ("phase".into(), json!("event")),
        ("flowStage".into(), json!("trigger")),
        ("durationMs".into(), json!(duration_ms)),
        ("status".into(), json!(if failed { "error" } else { "ok" })),
        ("errorStatus".into(), json!(failed)),
        ("httpMethod".into(), json!(request.method.as_str())),
        ("dataOperation".into(), json!("creatingRequestPayload")),
        ("rawRuntimeValuesRetained".into(), json!(false)),
    ]);
    let event = runtime.next_event(
        None,
        "trace.http.client.event",
        format!("Aone API client {}", request.method.as_str()),
        EvidenceKind::Observed,
        Some(context.trace_id.clone()),
        metadata,
    );
    publish_event(app, runtime, event);
}

pub(super) fn redact_headers(
    headers: &HeaderMap,
    secret_values: &[Zeroizing<String>],
) -> Vec<ApiHeader> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = if is_sensitive_name(name.as_str()) {
                "[REDACTED]".into()
            } else {
                redact_text(value.to_str().unwrap_or("[NON_UTF8]"), secret_values)
            };
            ApiHeader {
                name: name.as_str().to_string(),
                value,
            }
        })
        .collect()
}

fn request_header_names(request: &PreparedRequest) -> Vec<String> {
    request
        .headers
        .keys()
        .map(|name| name.as_str().to_string())
        .collect()
}

pub(super) fn redact_body(body: &str, secret_values: &[Zeroizing<String>]) -> String {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => serde_json::to_string(&redact_json(&value, secret_values))
            .unwrap_or_else(|_| "[UNSERIALIZABLE JSON]".into()),
        Err(_) => redact_text(body, secret_values),
    }
}

pub(super) fn redact_url(url: &Url) -> String {
    let mut redacted = url.clone();
    if url.query().is_some() {
        let pairs = url
            .query_pairs()
            .map(|(name, _)| (name.into_owned(), "[VALUE HIDDEN]".to_string()))
            .collect::<Vec<_>>();
        redacted.set_query(None);
        redacted.query_pairs_mut().extend_pairs(pairs);
    }
    redacted.to_string()
}

pub(super) fn is_hop_by_hop_request_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "content-length" | "transfer-encoding" | "connection" | "upgrade"
    )
}

pub(super) fn classify_request_error(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "HTTP request timed out"
    } else if error.is_connect() {
        "could not connect to the HTTP endpoint"
    } else if error.is_request() {
        "HTTP request could not be sent"
    } else {
        "HTTP request failed"
    }
}

pub(super) fn elapsed_millis(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
