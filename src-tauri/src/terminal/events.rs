use base64::{Engine as _, engine::general_purpose::STANDARD};
use tauri::{AppHandle, Emitter};

use super::time::timestamp_millis;
use crate::domain::{TerminalEvent, TerminalEventKind, TerminalOutputEncoding};

pub const TERMINAL_EVENT_NAME: &str = "aone-terminal-event";
const MAX_DETAIL_BYTES: usize = 1_024;

pub(super) fn publish_data(app: &AppHandle, session_id: &str, data: &[u8]) {
    publish(
        app,
        TerminalEvent {
            session_id: session_id.to_owned(),
            kind: TerminalEventKind::Data,
            timestamp: timestamp_millis(),
            data: Some(STANDARD.encode(data)),
            encoding: Some(TerminalOutputEncoding::Base64),
            byte_length: Some(data.len()),
            exit_code: None,
            detail: None,
        },
    );
}

pub(super) fn publish_exit(app: &AppHandle, session_id: &str, exit_code: u32) {
    publish(
        app,
        TerminalEvent {
            session_id: session_id.to_owned(),
            kind: TerminalEventKind::Exit,
            timestamp: timestamp_millis(),
            data: None,
            encoding: None,
            byte_length: None,
            exit_code: Some(exit_code),
            detail: None,
        },
    );
}

pub(super) fn publish_error(app: &AppHandle, session_id: &str, detail: &str) {
    publish(
        app,
        TerminalEvent {
            session_id: session_id.to_owned(),
            kind: TerminalEventKind::Error,
            timestamp: timestamp_millis(),
            data: None,
            encoding: None,
            byte_length: None,
            exit_code: None,
            detail: Some(bounded_detail(detail)),
        },
    );
}

fn publish(app: &AppHandle, event: TerminalEvent) {
    let _ = app.emit(TERMINAL_EVENT_NAME, event);
}

fn bounded_detail(value: &str) -> String {
    if value.len() <= MAX_DETAIL_BYTES {
        return value.to_owned();
    }
    let mut end = MAX_DETAIL_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end.saturating_sub(3)])
}
