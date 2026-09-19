use std::{
    net::Ipv4Addr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use super::{
    confirmation::{confirmation_message, resolve_after_approval},
    destination::ValidatedDestination,
    request::{MAX_MESSAGE_BYTES, PreparedWebSocket, outgoing_message, validate_session_id},
    state::{SessionControl, WebSocketState},
    time::timestamp_millis,
    transport::open_socket,
};
use crate::domain::{
    ApiHeader, WebSocketConnectRequest, WebSocketPayloadEncoding, WebSocketSendRequest,
};

fn connect_request(url: &str) -> WebSocketConnectRequest {
    WebSocketConnectRequest {
        url: url.into(),
        headers: Vec::new(),
        protocols: Vec::new(),
        timeout_ms: None,
    }
}

#[test]
fn prepares_a_bounded_ws_handshake_without_credentials() {
    let mut input = connect_request("wss://example.com/socket?token=hidden");
    input.headers.push(ApiHeader {
        name: "Authorization".into(),
        value: "Bearer secret".into(),
    });
    input.protocols = vec!["graphql-transport-ws".into()];
    input.timeout_ms = Some(250);
    let prepared = PreparedWebSocket::new(input).expect("valid request");
    assert_eq!(prepared.destination, "wss://example.com");
    assert_eq!(prepared.timeout_ms, 250);
    assert_eq!(prepared.header_names, vec!["authorization"]);
    let redaction_values = prepared.redaction_values();
    assert!(
        redaction_values
            .iter()
            .any(|value| value.as_str() == "Bearer secret")
    );
    assert!(
        redaction_values
            .iter()
            .any(|value| value.as_str() == "hidden")
    );
    let diagnostics = format!("{prepared:?}");
    assert!(!diagnostics.contains("Bearer secret"));
    assert!(!diagnostics.contains("token=hidden"));
    let handshake = prepared.handshake_request().expect("handshake request");
    assert_eq!(
        handshake.headers()["sec-websocket-protocol"],
        "graphql-transport-ws"
    );
}

#[test]
fn rejects_unsafe_or_unsupported_urls() {
    for url in [
        "https://example.com/socket",
        "file:///tmp/socket",
        "wss://user:password@example.com/socket",
        "wss://example.com/socket#secret",
    ] {
        assert!(
            PreparedWebSocket::new(connect_request(url)).is_err(),
            "{url} must be rejected"
        );
    }
}

#[test]
fn rejects_out_of_range_connection_timeouts() {
    for timeout_ms in [0, 249, 60_001, u64::MAX] {
        let mut input = connect_request("ws://localhost:9000");
        input.timeout_ms = Some(timeout_ms);
        assert!(PreparedWebSocket::new(input).is_err(), "{timeout_ms}");
    }
    let mut maximum = connect_request("ws://localhost:9000");
    maximum.timeout_ms = Some(60_000);
    assert_eq!(
        PreparedWebSocket::new(maximum)
            .expect("maximum timeout")
            .timeout_ms,
        60_000
    );
}

#[test]
fn serde_requires_contract_arrays_and_message_encoding() {
    assert!(
        serde_json::from_value::<WebSocketConnectRequest>(serde_json::json!({
            "url": "ws://localhost:9000"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WebSocketConnectRequest>(serde_json::json!({
            "url": "ws://localhost:9000",
            "headers": []
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WebSocketSendRequest>(serde_json::json!({
            "sessionId": "websocket:018f0000-0000-7000-8000-000000000000",
            "data": "hello"
        }))
        .is_err()
    );
}

#[test]
fn rejects_managed_duplicate_and_invalid_headers_without_echoing_values() {
    let cases = [
        vec![ApiHeader {
            name: "Host".into(),
            value: "sensitive.example".into(),
        }],
        vec![
            ApiHeader {
                name: "X-Test".into(),
                value: "first-secret".into(),
            },
            ApiHeader {
                name: "x-test".into(),
                value: "second-secret".into(),
            },
        ],
        vec![ApiHeader {
            name: "X-Test".into(),
            value: "secret\r\nInjected: true".into(),
        }],
    ];
    for headers in cases {
        let mut input = connect_request("ws://localhost:9000");
        input.headers = headers;
        let error = PreparedWebSocket::new(input)
            .expect_err("header must be rejected")
            .to_string();
        assert!(!error.contains("secret"));
    }
}

#[test]
fn validates_subprotocol_tokens_and_duplicates() {
    for protocols in [
        vec!["contains a space".into()],
        vec!["chat".into(), "chat".into()],
        vec!["x".repeat(129)],
    ] {
        let mut input = connect_request("ws://localhost:9000");
        input.protocols = protocols;
        assert!(PreparedWebSocket::new(input).is_err());
    }
}

#[test]
fn validates_text_and_base64_outbound_messages() {
    let session_id = format!("websocket:{}", uuid::Uuid::now_v7());
    let (_, text) = outgoing_message(WebSocketSendRequest {
        session_id: session_id.clone(),
        data: "hello".into(),
        encoding: WebSocketPayloadEncoding::Text,
    })
    .expect("text message");
    assert_eq!(text, Message::text("hello"));

    let (_, binary) = outgoing_message(WebSocketSendRequest {
        session_id,
        data: STANDARD.encode([0_u8, 1, 2]),
        encoding: WebSocketPayloadEncoding::Base64,
    })
    .expect("binary message");
    assert_eq!(binary, Message::binary(vec![0_u8, 1, 2]));
}

#[test]
fn rejects_invalid_sessions_base64_and_oversized_text() {
    assert!(validate_session_id("not-a-session").is_err());
    let session_id = format!("websocket:{}", uuid::Uuid::now_v7());
    assert!(
        outgoing_message(WebSocketSendRequest {
            session_id: session_id.clone(),
            data: "%%%".into(),
            encoding: WebSocketPayloadEncoding::Base64,
        })
        .is_err()
    );
    assert!(
        outgoing_message(WebSocketSendRequest {
            session_id,
            data: "x".repeat(MAX_MESSAGE_BYTES + 1),
            encoding: WebSocketPayloadEncoding::Text,
        })
        .is_err()
    );
}

#[test]
fn consent_shows_canonical_path_and_query_names_without_values() {
    let mut input = connect_request("wss://example.com/private/secret?token=hidden");
    input.headers.push(ApiHeader {
        name: "Authorization".into(),
        value: "Bearer super-secret".into(),
    });
    input.protocols.push("chat".into());
    let request = PreparedWebSocket::new(input).expect("valid request");
    let message = confirmation_message(&request);
    assert!(message.contains("wss://example.com:443/private/secret"));
    assert!(message.contains("token=%5Bvalue+hidden%5D"));
    assert!(message.contains("authorization"));
    assert!(message.contains("Requested subprotocols: 1"));
    assert!(!message.contains("super-secret"));
    assert!(!message.contains("token=hidden"));
    assert!(message.contains("network activity begin only after approval"));
}

#[test]
fn consent_warns_for_literal_private_destinations_without_resolution() {
    let request = PreparedWebSocket::new(connect_request("ws://127.0.0.1:9000/events"))
        .expect("literal request");
    assert!(confirmation_message(&request).contains("WARNING"));
}

#[tokio::test]
async fn declined_consent_does_not_invoke_the_websocket_resolver() {
    let calls = Arc::new(AtomicUsize::new(0));
    let resolver_calls = Arc::clone(&calls);
    let result = resolve_after_approval(
        async {
            Err(crate::error::AoneError::InvalidRequest(
                "WebSocket connection was cancelled".into(),
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

#[test]
fn registry_enforces_slots_uniqueness_and_shutdown() {
    let state = WebSocketState::with_capacity(1);
    let registry = state.registry();
    let slot = registry.reserve_slot().expect("first slot");
    assert!(registry.reserve_slot().is_err());
    drop(slot);
    assert!(registry.reserve_slot().is_ok());

    let (outbound, _) = mpsc::channel(1);
    let (shutdown, receiver) = watch::channel(false);
    registry
        .register("session".into(), SessionControl { outbound, shutdown })
        .expect("register session");
    assert_eq!(registry.active_count(), 1);
    let (duplicate_outbound, _) = mpsc::channel(1);
    let (duplicate_shutdown, _) = watch::channel(false);
    assert!(
        registry
            .register(
                "session".into(),
                SessionControl {
                    outbound: duplicate_outbound,
                    shutdown: duplicate_shutdown,
                },
            )
            .is_err()
    );
    assert!(registry.outbound_sender("session").is_some());
    assert!(registry.disconnect("session"));
    assert!(*receiver.borrow());
    assert_eq!(registry.active_count(), 0);
    assert!(!registry.disconnect("session"));
}

#[test]
fn websocket_timestamps_are_rfc3339_utc() {
    let timestamp = timestamp_millis();
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
    assert_eq!(timestamp.len(), 24);
}

#[tokio::test]
async fn opens_a_direct_validated_socket_and_exchanges_messages() {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept TCP");
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept WebSocket");
        assert_eq!(
            socket
                .next()
                .await
                .expect("message")
                .expect("valid message"),
            Message::text("hello")
        );
        socket
            .send(Message::text("ack"))
            .await
            .expect("send response");
    });

    let request = PreparedWebSocket::new(connect_request(&format!(
        "ws://socket.invalid:{}/events",
        address.port()
    )))
    .expect("prepared request");
    let destination = ValidatedDestination {
        addresses: vec![address],
    };
    let (mut socket, protocol) = open_socket(&request, &destination)
        .await
        .expect("open direct pinned socket");
    assert!(protocol.is_none());
    socket
        .send(Message::text("hello"))
        .await
        .expect("send message");
    assert_eq!(
        socket
            .next()
            .await
            .expect("response")
            .expect("valid response"),
        Message::text("ack")
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn rejects_handshake_redirects_without_following_location() {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind redirect server");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept TCP");
        let mut request = [0_u8; 4_096];
        let _ = stream.read(&mut request).await.expect("read handshake");
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: ws://169.254.169.254/latest\r\nContent-Length: 0\r\n\r\n",
            )
            .await
            .expect("write redirect");
    });
    let request = PreparedWebSocket::new(connect_request(&format!(
        "ws://socket.invalid:{}/events",
        address.port()
    )))
    .expect("prepared request");
    let destination = ValidatedDestination {
        addresses: vec![address],
    };
    let error = open_socket(&request, &destination)
        .await
        .expect_err("redirect must fail")
        .to_string();
    assert!(error.contains("redirects are disabled"));
    server.await.expect("server task");
}
