use base64::{Engine as _, engine::general_purpose::STANDARD};
use tauri::{AppHandle, Emitter};
use tokio_tungstenite::tungstenite::Message;

use super::{redaction::SecretRedactor, time::timestamp_millis};
use crate::domain::{WebSocketEvent, WebSocketEventKind, WebSocketPayloadEncoding};

pub const WEBSOCKET_EVENT_NAME: &str = "aone-websocket-event";
const MAX_TEXT_PREVIEW_BYTES: usize = 256 * 1024;
const MAX_BINARY_PREVIEW_BYTES: usize = 192 * 1024;
const MAX_DETAIL_BYTES: usize = 1_024;

pub(super) fn publish(app: &AppHandle, event: WebSocketEvent) {
    let _ = app.emit(WEBSOCKET_EVENT_NAME, event);
}

pub(super) fn lifecycle_event(
    session_id: &str,
    kind: WebSocketEventKind,
    destination: &str,
    protocol: Option<String>,
    detail: Option<&str>,
    redactor: &SecretRedactor,
) -> WebSocketEvent {
    WebSocketEvent {
        session_id: session_id.to_owned(),
        kind,
        timestamp: timestamp_millis(),
        destination: destination.to_owned(),
        protocol,
        data: None,
        encoding: None,
        byte_length: None,
        dropped_count: None,
        truncated: false,
        detail: detail.map(|value| redactor.text_preview(value, MAX_DETAIL_BYTES).0),
    }
}

pub(super) fn dropped_event(
    session_id: &str,
    destination: &str,
    protocol: Option<String>,
    dropped_count: u64,
) -> WebSocketEvent {
    WebSocketEvent {
        session_id: session_id.to_owned(),
        kind: WebSocketEventKind::Dropped,
        timestamp: timestamp_millis(),
        destination: destination.to_owned(),
        protocol,
        data: None,
        encoding: None,
        byte_length: None,
        dropped_count: Some(dropped_count),
        truncated: false,
        detail: Some(format!(
            "{dropped_count} inbound WebSocket messages were omitted by the event rate limit"
        )),
    }
}

pub(super) fn message_event(
    session_id: &str,
    destination: &str,
    protocol: Option<String>,
    message: &Message,
    redactor: &SecretRedactor,
) -> Option<WebSocketEvent> {
    let (data, encoding, byte_length, truncated) = match message {
        Message::Text(text) => {
            let byte_length = text.len();
            let (data, preview_truncated) =
                redactor.text_preview(text.as_str(), MAX_TEXT_PREVIEW_BYTES);
            (
                data,
                WebSocketPayloadEncoding::Text,
                byte_length,
                preview_truncated,
            )
        }
        Message::Binary(data) => {
            let byte_length = data.len();
            let (redacted, preview_truncated) =
                redactor.binary_preview(data, MAX_BINARY_PREVIEW_BYTES);
            (
                STANDARD.encode(redacted),
                WebSocketPayloadEncoding::Base64,
                byte_length,
                preview_truncated,
            )
        }
        _ => return None,
    };
    Some(WebSocketEvent {
        session_id: session_id.to_owned(),
        kind: WebSocketEventKind::Message,
        timestamp: timestamp_millis(),
        destination: destination.to_owned(),
        protocol,
        data: Some(data),
        encoding: Some(encoding),
        byte_length: Some(byte_length),
        dropped_count: None,
        truncated,
        detail: None,
    })
}
