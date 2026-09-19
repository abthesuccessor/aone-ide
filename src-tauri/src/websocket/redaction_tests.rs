use base64::{Engine as _, engine::general_purpose::STANDARD};
use tokio_tungstenite::tungstenite::Message;
use zeroize::Zeroizing;

use super::{
    events::{dropped_event, message_event},
    redaction::{MAX_SECRET_PATTERNS, RedactionCache, SecretRedactor},
    request::PreparedWebSocket,
};
use crate::domain::{ApiHeader, WebSocketConnectRequest, WebSocketEventKind};

fn redactor(values: &[&str]) -> SecretRedactor {
    SecretRedactor::from_values(
        &values
            .iter()
            .map(|value| Zeroizing::new((*value).to_owned()))
            .collect::<Vec<_>>(),
    )
}

fn text_event(value: &str, redactor: &SecretRedactor) -> crate::domain::WebSocketEvent {
    message_event(
        "websocket:018f0000-0000-7000-8000-000000000000",
        "wss://example.com",
        None,
        &Message::text(value),
        redactor,
    )
    .expect("text event")
}

#[test]
fn message_events_redact_known_text_and_binary_secrets() {
    let redactor = redactor(&["open-sesame"]);
    let event = text_event("token: open-sesame", &redactor);
    assert_eq!(event.kind, WebSocketEventKind::Message);
    assert_eq!(event.data.as_deref(), Some("token: [REDACTED]"));
    assert!(!event.truncated);

    let binary = Message::binary(b"prefix-open-sesame-suffix".to_vec());
    let event =
        message_event("id", "ws://localhost", None, &binary, &redactor).expect("binary event");
    let decoded = STANDARD
        .decode(event.data.expect("event data"))
        .expect("base64");
    assert!(!decoded.windows(11).any(|value| value == b"open-sesame"));
}

#[test]
fn inbound_event_previews_are_strictly_bounded() {
    let empty = redactor(&[]);
    let text_event = text_event(&"x".repeat(300_000), &empty);
    assert!(text_event.truncated);
    assert_eq!(text_event.byte_length, Some(300_000));
    assert!(text_event.data.expect("text preview").len() <= 256 * 1024);

    let binary = Message::binary(vec![7_u8; 256 * 1024]);
    let event =
        message_event("id", "wss://example.com", None, &binary, &empty).expect("binary event");
    assert!(event.truncated);
    assert_eq!(event.byte_length, Some(256 * 1024));
    assert!(event.data.expect("binary preview").len() <= 256 * 1024);
}

#[test]
fn request_secrets_are_redacted_from_inbound_events() {
    let request = PreparedWebSocket::new(WebSocketConnectRequest {
        url: "wss://example.com/events?access_token=query-secret".into(),
        headers: vec![ApiHeader {
            name: "Authorization".into(),
            value: "Bearer header-secret".into(),
        }],
        protocols: vec![],
        timeout_ms: None,
    })
    .expect("prepared request");
    let redactor = SecretRedactor::from_values(&request.redaction_values());
    let event = text_event("Bearer header-secret and query-secret", &redactor);
    assert_eq!(event.data.as_deref(), Some("[REDACTED] and [REDACTED]"));
}

#[test]
fn leftmost_longest_handles_overlapping_text_and_binary_patterns() {
    let redactor = redactor(&["token", "token-long", "aba", "ababa"]);
    let text = text_event("token-long ababa", &redactor)
        .data
        .expect("text data");
    assert_eq!(text, "[REDACTED] [REDACTED]");
    assert!(!text.contains("-long"));

    let binary = Message::binary(b"ababa-token-long".to_vec());
    let event =
        message_event("id", "wss://example.com", None, &binary, &redactor).expect("binary event");
    let decoded = STANDARD
        .decode(event.data.expect("binary data"))
        .expect("base64");
    assert_eq!(decoded, b"[REDACTED]-[REDACTED]");
}

#[test]
fn recursively_redacts_sensitive_json_fields_and_known_values() {
    let redactor = redactor(&["ordinary-secret"]);
    let event = text_event(
        r#"{"safe":{"password":"top-secret","items":[{"apiKey":"key"}]},"token":123,"ordinary":"ordinary-secret"}"#,
        &redactor,
    );
    let value: serde_json::Value =
        serde_json::from_str(event.data.as_deref().expect("JSON event")).expect("valid JSON");
    assert_eq!(value["safe"]["password"], "[REDACTED]");
    assert_eq!(value["safe"]["items"][0]["apiKey"], "[REDACTED]");
    assert_eq!(value["token"], "[REDACTED]");
    assert_eq!(value["ordinary"], "[REDACTED]");
}

#[test]
fn redacts_known_values_used_as_json_keys_or_numeric_scalars() {
    let key_event = text_event(r#"{"opaque-secret":"safe"}"#, &redactor(&["opaque-secret"]));
    let key_preview = key_event.data.expect("key preview");
    assert!(!key_preview.contains("opaque-secret"));
    assert!(key_preview.contains("[REDACTED]"));

    let numeric_event = text_event(r#"{"value":24681357}"#, &redactor(&["24681357"]));
    let numeric_preview = numeric_event.data.expect("numeric preview");
    assert!(!numeric_preview.contains("24681357"));
    assert!(numeric_preview.contains("CONTENT HIDDEN"));
    assert!(numeric_event.truncated);
}

#[test]
fn redacts_secrets_across_preview_boundaries_before_truncation() {
    let limit = 192 * 1024;
    let redactor = redactor(&["boundary-secret"]);
    let mut data = vec![b'x'; limit - 4];
    data.extend_from_slice(b"boundary-secret");
    data.extend_from_slice(b"tail");
    let event = message_event(
        "id",
        "wss://example.com",
        None,
        &Message::binary(data),
        &redactor,
    )
    .expect("binary event");
    let preview = STANDARD
        .decode(event.data.expect("base64 preview"))
        .expect("base64");
    assert!(event.truncated);
    assert!(!preview.ends_with(b"boun"));
    assert!(!preview.windows(15).any(|value| value == b"boundary-secret"));
}

#[test]
fn capped_writer_reports_redaction_expansion_without_allocating_full_output() {
    let redactor = redactor(&["x"]);
    let event = message_event(
        "id",
        "wss://example.com",
        None,
        &Message::binary(vec![b'x'; 30_000]),
        &redactor,
    )
    .expect("binary event");
    assert_eq!(event.byte_length, Some(30_000));
    assert!(event.truncated);
    let preview = STANDARD
        .decode(event.data.expect("base64 preview"))
        .expect("base64");
    assert_eq!(preview.len(), 192 * 1024);
}

#[test]
fn unicode_truncation_keeps_valid_boundaries() {
    let (value, truncated) = redactor(&[]).text_preview("ab🦀cd", 5);
    assert_eq!(value, "ab");
    assert!(truncated);
}

#[test]
fn pattern_caps_dedupe_and_fail_closed_without_leaking_payload() {
    let duplicates = (0..=MAX_SECRET_PATTERNS)
        .map(|_| Zeroizing::new("same-secret".to_owned()))
        .collect::<Vec<_>>();
    let deduped = SecretRedactor::from_values(&duplicates);
    assert_eq!(deduped.text_preview("same-secret", 128).0, "[REDACTED]");

    let excessive = (0..=MAX_SECRET_PATTERNS)
        .map(|index| Zeroizing::new(format!("secret-{index}")))
        .collect::<Vec<_>>();
    let fail_closed = SecretRedactor::from_values(&excessive);
    let output = fail_closed.text_preview("ordinary payload", 128);
    assert!(output.0.contains("CONTENT HIDDEN"));
    assert!(output.1);
    assert!(!output.0.contains("ordinary payload"));
}

#[test]
fn redaction_cache_refreshes_when_dynamic_secrets_change() {
    let mut cache = RedactionCache::new(vec![Zeroizing::new("persistent".into())]);
    let first = vec![Zeroizing::new("first".into())];
    assert_eq!(
        cache
            .refresh(&first)
            .text_preview("persistent first", 128)
            .0,
        "[REDACTED] [REDACTED]"
    );
    let second = vec![Zeroizing::new("second".into())];
    assert_eq!(
        cache
            .refresh(&second)
            .text_preview("persistent second", 128)
            .0,
        "[REDACTED] [REDACTED]"
    );
}

#[test]
fn dropped_event_has_an_explicit_count_without_payload() {
    let event = dropped_event("id", "wss://example.com", None, 42);
    assert_eq!(event.kind, WebSocketEventKind::Dropped);
    assert_eq!(event.dropped_count, Some(42));
    assert!(event.data.is_none());
}
