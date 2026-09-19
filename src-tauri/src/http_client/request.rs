use std::{str::FromStr, time::Duration};

use reqwest::{
    Client, Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

use super::{
    confirmation::{
        OutboundAuthorization, confirm_outbound_request, outbound_authorization,
        resolve_after_approval,
    },
    destination::ValidatedDestination,
    response::{
        classify_request_error, elapsed_millis, emit_request_completed, emit_request_failed,
        emit_request_started, is_hop_by_hop_request_header, read_response_bounded, redact_body,
        redact_headers,
    },
};
use crate::{
    domain::{ApiRequest, ApiResponse},
    error::{AoneError, AoneResult},
    runner::RuntimeState,
    state::{AppState, WorkspaceContext},
};

const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 60_000;
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub(super) const MAX_HEADERS: usize = 128;
pub(super) const MAX_URL_BYTES: usize = 16 * 1024;
pub(super) const MAX_HEADER_BYTES: usize = 64 * 1024;

#[tauri::command]
pub async fn send_api_request(
    request: ApiRequest,
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
    app_state: State<'_, AppState>,
) -> AoneResult<ApiResponse> {
    let workspace = app_state.workspace()?;
    let mut prepared = PreparedRequest::new(request)?;
    let authorization = outbound_authorization(&prepared.url);
    let destination = match authorization {
        OutboundAuthorization::ExplicitLoopbackSend => {
            ValidatedDestination::resolve(&prepared.url).await?
        }
        OutboundAuthorization::NativeConfirmation => {
            resolve_after_approval(confirm_outbound_request(&app, &prepared), || {
                ValidatedDestination::resolve(&prepared.url)
            })
            .await?
        }
    };
    let receiver_active = runtime.otlp.lock().active_for_workspace(&workspace.id);
    prepared.inject_trace_context(receiver_active && destination.is_entirely_local_or_private());

    let request_id = format!("request:{}", Uuid::now_v7());
    let secret_values = runtime.secret_values_for_redaction();
    emit_for_workspace(&app_state, &runtime, &workspace, || {
        emit_request_started(&app, &runtime, &request_id, &prepared)
    })?;

    let mut client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_millis(prepared.timeout_ms))
        // An ambient proxy could bypass the validated destination and disclose
        // API Explorer traffic. v0.1 therefore connects directly.
        .no_proxy();
    if destination.was_dns_resolved {
        client = client.resolve_to_addrs(&destination.host, &destination.addresses);
    }
    let client = client
        .build()
        .map_err(|error| AoneError::Task(format!("could not initialize HTTP client: {error}")))?;

    let started = std::time::Instant::now();
    let mut builder = client
        .request(prepared.method.clone(), prepared.url.clone())
        .headers(prepared.headers.clone())
        .timeout(Duration::from_millis(prepared.timeout_ms));
    if let Some(body) = &prepared.body {
        builder = builder.body(body.clone());
    }

    let response = match builder.send().await {
        Ok(response) => response,
        Err(error) => {
            let duration_ms = elapsed_millis(started);
            let message = classify_request_error(&error);
            emit_for_workspace(&app_state, &runtime, &workspace, || {
                emit_request_failed(&app, &runtime, &request_id, &prepared, duration_ms, message)
            })?;
            return Err(AoneError::Task(message.into()));
        }
    };

    let status = response.status();
    let response_headers = response.headers().clone();
    let (body_bytes, truncated) = read_response_bounded(response).await?;
    let duration_ms = elapsed_millis(started);
    let raw_body = String::from_utf8_lossy(&body_bytes).into_owned();
    let body = redact_body(&raw_body, &secret_values);
    let headers = redact_headers(&response_headers, &secret_values);
    let result = ApiResponse {
        request_id: request_id.clone(),
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or_default().to_string(),
        headers,
        body,
        duration_ms,
        truncated,
    };

    emit_for_workspace(&app_state, &runtime, &workspace, || {
        emit_request_completed(&app, &runtime, &prepared, &result)
    })?;
    Ok(result)
}

pub(super) fn emit_for_workspace(
    app_state: &AppState,
    runtime: &RuntimeState,
    workspace: &WorkspaceContext,
    emit: impl FnOnce(),
) -> AoneResult<()> {
    app_state.while_workspace_current(&workspace.id, || {
        runtime.with_workspace_current(&workspace.root, emit)
    })
}

#[derive(Debug)]
pub(super) struct PreparedRequest {
    pub(super) method: Method,
    pub(super) url: Url,
    pub(super) headers: HeaderMap,
    pub(super) body: Option<String>,
    pub(super) timeout_ms: u64,
    pub(super) trace_context: Option<TraceContext>,
}

#[derive(Debug, Clone)]
pub(super) struct TraceContext {
    pub(super) trace_id: String,
    pub(super) span_id: String,
}

impl PreparedRequest {
    pub(super) fn new(request: ApiRequest) -> AoneResult<Self> {
        if request.url.len() > MAX_URL_BYTES {
            return Err(AoneError::InvalidRequest(format!(
                "request URL exceeds {MAX_URL_BYTES} bytes"
            )));
        }
        let method = Method::from_str(request.method.trim())
            .map_err(|_| AoneError::InvalidRequest("invalid HTTP method".into()))?;
        if matches!(method, Method::CONNECT | Method::TRACE) {
            return Err(AoneError::InvalidRequest(
                "CONNECT and TRACE are not supported by API Explorer".into(),
            ));
        }
        let url = Url::parse(request.url.trim())
            .map_err(|_| AoneError::InvalidRequest("invalid request URL".into()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AoneError::InvalidRequest(
                "only http and https URLs are supported".into(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AoneError::InvalidRequest(
                "credentials in request URLs are not allowed; use a header".into(),
            ));
        }
        if url.host_str().is_none() {
            return Err(AoneError::InvalidRequest(
                "request URL must include a host".into(),
            ));
        }
        if url.fragment().is_some() {
            return Err(AoneError::InvalidRequest(
                "URL fragments are not sent to servers and are not supported".into(),
            ));
        }
        if url.as_str().len() > MAX_URL_BYTES {
            return Err(AoneError::InvalidRequest(format!(
                "canonical request URL exceeds {MAX_URL_BYTES} bytes"
            )));
        }
        if request.headers.len() > MAX_HEADERS {
            return Err(AoneError::InvalidRequest("too many request headers".into()));
        }

        let mut headers = HeaderMap::new();
        let mut aggregate_header_bytes = 0_usize;
        for header in request.headers {
            aggregate_header_bytes = aggregate_header_bytes
                .checked_add(header.name.len())
                .and_then(|total| total.checked_add(header.value.len()))
                .ok_or_else(|| AoneError::InvalidRequest("request headers are too large".into()))?;
            if aggregate_header_bytes > MAX_HEADER_BYTES {
                return Err(AoneError::InvalidRequest(format!(
                    "request headers exceed {MAX_HEADER_BYTES} aggregate bytes"
                )));
            }
            let name = HeaderName::from_str(header.name.trim())
                .map_err(|_| AoneError::InvalidRequest("invalid HTTP header name".into()))?;
            if is_hop_by_hop_request_header(name.as_str()) {
                return Err(AoneError::InvalidRequest(format!(
                    "header {} is managed by the HTTP client",
                    name.as_str()
                )));
            }
            let value = HeaderValue::from_str(&header.value)
                .map_err(|_| AoneError::InvalidRequest("invalid HTTP header value".into()))?;
            headers.append(name, value);
        }

        let body = match request.body {
            Some(body) => {
                if body.len() > MAX_REQUEST_BODY_BYTES {
                    return Err(AoneError::InvalidRequest(format!(
                        "request body exceeds {MAX_REQUEST_BODY_BYTES} bytes"
                    )));
                }
                // v0.1 intentionally accepts JSON only. GraphQL requests use the
                // standard {query, variables, operationName} JSON envelope.
                let parsed: Value = serde_json::from_str(&body).map_err(|_| {
                    AoneError::InvalidRequest("request body must be valid JSON".into())
                })?;
                if !headers.contains_key(reqwest::header::CONTENT_TYPE) {
                    headers.insert(
                        reqwest::header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    );
                }
                Some(serde_json::to_string(&parsed)?)
            }
            None => None,
        };

        let encoded_header_bytes = headers.iter().try_fold(0_usize, |total, (name, value)| {
            total
                .checked_add(name.as_str().len())
                .and_then(|sum| sum.checked_add(value.as_bytes().len()))
        });
        if encoded_header_bytes.is_none_or(|bytes| bytes > MAX_HEADER_BYTES) {
            return Err(AoneError::InvalidRequest(format!(
                "request headers exceed {MAX_HEADER_BYTES} aggregate bytes"
            )));
        }

        let timeout_ms = request
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);

        Ok(Self {
            method,
            url,
            headers,
            body,
            timeout_ms,
            trace_context: None,
        })
    }

    pub(super) fn inject_trace_context(&mut self, allowed: bool) -> bool {
        if !allowed || self.headers.contains_key("traceparent") || self.headers.len() >= MAX_HEADERS
        {
            return false;
        }
        let current_header_bytes = self.headers.iter().fold(0_usize, |total, (name, value)| {
            total
                .saturating_add(name.as_str().len())
                .saturating_add(value.as_bytes().len())
        });
        if current_header_bytes.saturating_add(66) > MAX_HEADER_BYTES {
            return false;
        }
        let trace_id = Uuid::new_v4().simple().to_string();
        let span_id = Uuid::new_v4().simple().to_string()[..16].to_owned();
        let value = format!("00-{trace_id}-{span_id}-01");
        let Ok(value) = HeaderValue::from_str(&value) else {
            return false;
        };
        self.headers
            .insert(HeaderName::from_static("traceparent"), value);
        self.trace_context = Some(TraceContext { trace_id, span_id });
        true
    }
}
