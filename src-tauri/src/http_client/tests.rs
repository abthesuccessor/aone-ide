use std::{
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use reqwest::{Method, Url};
use zeroize::Zeroizing;

use super::{
    confirmation::{
        OutboundAuthorization, await_outbound_confirmation, outbound_authorization,
        outbound_confirmation_message, resolve_after_approval,
    },
    destination::{ValidatedDestination, literal_loopback, literal_private_warning},
    request::{MAX_HEADER_BYTES, MAX_URL_BYTES, PreparedRequest},
    response::{
        redact_body, redact_headers, redact_url, request_event_metadata, response_event_metadata,
    },
};
use crate::domain::{ApiHeader, ApiRequest, ApiResponse};
use crate::runner::RuntimeState;

fn request(body: Option<&str>) -> ApiRequest {
    ApiRequest {
        method: "POST".into(),
        url: "http://127.0.0.1:3000/graphql?token=secret&view=full".into(),
        headers: vec![ApiHeader {
            name: "Authorization".into(),
            value: "Bearer secret".into(),
        }],
        body: body.map(str::to_string),
        timeout_ms: Some(500),
    }
}

#[test]
fn accepts_graphql_json_envelope() {
    let prepared = PreparedRequest::new(request(Some(
        r#"{"query":"query Thing { thing { id } }","variables":{"id":1}}"#,
    )))
    .unwrap();
    assert_eq!(prepared.method, Method::POST);
    assert_eq!(
        prepared.headers.get(reqwest::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
}

#[test]
fn rejects_non_json_request_bodies() {
    let error = PreparedRequest::new(request(Some("not JSON"))).unwrap_err();
    assert!(error.to_string().contains("valid JSON"));
}

#[test]
fn rejects_credentials_embedded_in_url() {
    let mut input = request(None);
    input.url = "https://user:password@example.com/".into();
    let error = PreparedRequest::new(input).unwrap_err();
    assert!(error.to_string().contains("credentials in request URLs"));
}

#[test]
fn redacts_sensitive_headers_and_nested_json() {
    let prepared = PreparedRequest::new(request(Some(
        r#"{"password":"hidden","nested":{"safe":"secret-value"}}"#,
    )))
    .unwrap();
    let secrets = vec![Zeroizing::new("secret-value".to_string())];
    let headers = redact_headers(&prepared.headers, &secrets);
    assert_eq!(
        headers
            .iter()
            .find(|header| header.name == "authorization")
            .unwrap()
            .value,
        "[REDACTED]"
    );
    let body = redact_body(prepared.body.as_deref().unwrap(), &secrets);
    assert!(body.contains(r#""password":"[REDACTED]""#));
    assert!(body.contains(r#""safe":"[REDACTED]""#));
}

#[test]
fn redacts_sensitive_url_query_values() {
    let url = Url::parse("https://example.com/path?api_key=abc&opaque=private-value").unwrap();
    let value = redact_url(&url);
    assert!(!value.contains("abc"));
    assert!(!value.contains("private-value"));
    assert!(value.contains("api_key="));
    assert!(value.contains("opaque="));
}

#[test]
fn runtime_events_never_retain_header_or_body_values() {
    let prepared = PreparedRequest::new(request(Some(
        r#"{"ordinaryCustomerField":"private-business-value"}"#,
    )))
    .unwrap();
    let started = request_event_metadata(&prepared);
    let completed = response_event_metadata(
        &prepared,
        &ApiResponse {
            request_id: "request:test".into(),
            status: 200,
            status_text: "OK".into(),
            headers: vec![ApiHeader {
                name: "x-business-context".into(),
                value: "private-header-value".into(),
            }],
            body: r#"{"customer":"private-response-value"}"#.into(),
            duration_ms: 12,
            truncated: false,
        },
    );
    let serialized = serde_json::to_string(&(started, completed)).unwrap();
    assert!(!serialized.contains("private-business-value"));
    assert!(!serialized.contains("private-header-value"));
    assert!(!serialized.contains("private-response-value"));
    assert!(!serialized.contains("ordinaryCustomerField"));
    assert!(!serialized.contains("secret"));
    assert!(serialized.contains("headerNames"));
    assert!(serialized.contains("bodyBytes"));
}

#[tokio::test]
async fn literal_private_destination_is_allowed_only_as_a_warned_target() {
    let url = Url::parse("http://127.0.0.1:3000/health").unwrap();
    let destination = ValidatedDestination::resolve(&url).await.unwrap();
    assert!(!destination.was_dns_resolved);
    assert!(literal_loopback(&url));
    assert!(literal_private_warning(&url));
    assert_eq!(
        destination.addresses[0].ip(),
        "127.0.0.1".parse::<IpAddr>().unwrap()
    );

    let ipv6_url = Url::parse("http://[::1]:3000/health").unwrap();
    let ipv6_destination = ValidatedDestination::resolve(&ipv6_url).await.unwrap();
    assert!(!ipv6_destination.was_dns_resolved);
    assert!(literal_loopback(&ipv6_url));
    assert!(literal_private_warning(&ipv6_url));
    assert_eq!(
        ipv6_destination.addresses[0].ip(),
        "::1".parse::<IpAddr>().unwrap()
    );

    // The URL parser canonicalizes legacy numeric IPv4 spellings before
    // destination validation, preventing a loopback classification bypass.
    let legacy_ipv4 = Url::parse("http://2130706433:3000/health").unwrap();
    let legacy_destination = ValidatedDestination::resolve(&legacy_ipv4).await.unwrap();
    assert!(literal_loopback(&legacy_ipv4));
    assert!(literal_private_warning(&legacy_ipv4));
    assert_eq!(
        legacy_destination.addresses[0].ip(),
        "127.0.0.1".parse::<IpAddr>().unwrap()
    );
}

#[test]
fn explicit_send_bypasses_confirmation_only_for_ip_literal_loopback() {
    let cases = [
        (
            "http://127.0.0.1:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://127.42.9.8:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://[::1]:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://[::ffff:127.0.0.1]:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://[::127.0.0.1]:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://2130706433:3000/health",
            OutboundAuthorization::ExplicitLoopbackSend,
        ),
        (
            "http://localhost:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "http://10.0.0.1:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "http://[fd12::1]:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "http://[::ffff:10.0.0.1]:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "http://[::ffff:8.8.8.8]:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "http://[::ffff:169.254.169.254]:3000/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "https://8.8.8.8/health",
            OutboundAuthorization::NativeConfirmation,
        ),
        (
            "https://example.com/health",
            OutboundAuthorization::NativeConfirmation,
        ),
    ];

    for (target, expected) in cases {
        let url = Url::parse(target).unwrap();
        assert_eq!(
            outbound_authorization(&url),
            expected,
            "unexpected authorization for {target}"
        );
    }
}

#[tokio::test]
async fn trace_context_is_injected_only_for_local_or_private_destinations() {
    let mut prepared = PreparedRequest::new(request(None)).unwrap();
    let local = ValidatedDestination::resolve(&prepared.url).await.unwrap();
    assert!(prepared.inject_trace_context(local.is_entirely_local_or_private()));
    let traceparent = prepared
        .headers
        .get("traceparent")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(traceparent.len(), 55);
    assert!(traceparent.starts_with("00-"));
    assert!(traceparent.ends_with("-01"));
    assert_eq!(prepared.trace_context.as_ref().unwrap().trace_id.len(), 32);
    assert_eq!(prepared.trace_context.as_ref().unwrap().span_id.len(), 16);

    let public = ValidatedDestination {
        host: "example.com".into(),
        addresses: vec!["8.8.8.8:443".parse().unwrap()],
        was_dns_resolved: true,
    };
    let mut public_request = PreparedRequest::new(request(None)).unwrap();
    assert!(!public_request.inject_trace_context(public.is_entirely_local_or_private()));
    assert!(!public_request.headers.contains_key("traceparent"));
}

#[tokio::test]
async fn link_local_and_metadata_destinations_are_rejected() {
    let link_local = Url::parse("http://169.254.20.1/latest").unwrap();
    assert!(ValidatedDestination::resolve(&link_local).await.is_err());

    let metadata = Url::parse("http://metadata.google.internal/computeMetadata/v1").unwrap();
    let error = ValidatedDestination::resolve(&metadata).await.unwrap_err();
    assert!(error.to_string().contains("cloud metadata"));

    let ipv6_metadata = Url::parse("http://[fd00:ec2::254]/latest").unwrap();
    assert!(ValidatedDestination::resolve(&ipv6_metadata).await.is_err());
}

#[test]
fn request_limits_url_and_aggregate_header_bytes() {
    let mut oversized_url = request(None);
    oversized_url.url = format!("https://example.com/{}", "a".repeat(MAX_URL_BYTES));
    assert!(
        PreparedRequest::new(oversized_url)
            .unwrap_err()
            .to_string()
            .contains("URL exceeds")
    );

    let mut oversized_headers = request(None);
    oversized_headers.headers = vec![ApiHeader {
        name: "X-Large".into(),
        value: "a".repeat(MAX_HEADER_BYTES),
    }];
    assert!(
        PreparedRequest::new(oversized_headers)
            .unwrap_err()
            .to_string()
            .contains("aggregate bytes")
    );
}

#[test]
fn confirmation_shows_canonical_target_without_header_body_or_query_values() {
    let prepared = PreparedRequest::new(request(Some(r#"{"password":"body-secret"}"#))).unwrap();
    let message = outbound_confirmation_message(&prepared);
    assert!(message.contains("Method: POST"));
    assert!(message.contains("Origin: http://127.0.0.1:3000"));
    assert!(message.contains("token=%5Bvalue+hidden%5D"));
    assert!(message.contains("WARNING:"));
    assert!(!message.contains("Bearer secret"));
    assert!(!message.contains("body-secret"));
    assert!(!message.contains("token=secret"));
    assert!(message.contains("network activity begin only after approval"));
}

#[tokio::test]
async fn declined_consent_does_not_invoke_the_http_resolver() {
    let calls = Arc::new(AtomicUsize::new(0));
    let resolver_calls = Arc::clone(&calls);
    let result = resolve_after_approval(
        async {
            Err(crate::error::AoneError::InvalidRequest(
                "outbound API request was cancelled".into(),
            ))
        },
        move || async move {
            resolver_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn confirmation_timeout_fails_closed_without_invoking_the_http_resolver() {
    let calls = Arc::new(AtomicUsize::new(0));
    let resolver_calls = Arc::clone(&calls);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let result = resolve_after_approval(
        await_outbound_confirmation(receiver, Duration::ZERO),
        move || async move {
            resolver_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    )
    .await;
    drop(sender);

    let error = result.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("timed out before network activity")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn request_completion_emission_is_suppressed_after_workspace_switch() {
    use crate::{
        state::{AppState, new_workspace_context},
        store::GraphStore,
    };

    let app_data = tempfile::tempdir().unwrap();
    let app_state = AppState::new(app_data.path().to_path_buf()).unwrap();
    let runtime = RuntimeState::default();
    let first = tempfile::tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    let first_database = tempfile::tempdir().unwrap();
    let first_workspace = new_workspace_context(
        "workspace:first".into(),
        first_root.clone(),
        GraphStore::open(&first_database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();
    app_state.install_workspace_for_test(first_workspace.clone());
    runtime.begin_workspace_switch().unwrap();
    runtime.complete_workspace_switch(first_root.clone());

    runtime.begin_workspace_switch().unwrap();
    let second = tempfile::tempdir().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    let second_database = tempfile::tempdir().unwrap();
    let second_workspace = new_workspace_context(
        "workspace:second".into(),
        second_root.clone(),
        GraphStore::open(&second_database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();
    app_state.install_workspace_for_test(second_workspace);
    runtime.complete_workspace_switch(second_root);

    let emitted = std::cell::Cell::new(false);
    let result = super::request::emit_for_workspace(&app_state, &runtime, &first_workspace, || {
        emitted.set(true)
    });
    assert!(result.is_err());
    assert!(!emitted.get());
    assert!(runtime.list_events(None).is_empty());
}
