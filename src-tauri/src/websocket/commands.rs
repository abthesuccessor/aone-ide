use tauri::{AppHandle, State};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use super::{
    confirmation::{confirm_connection, resolve_after_approval},
    destination::ValidatedDestination,
    events::{lifecycle_event, publish},
    redaction::SecretRedactor,
    request::{PreparedWebSocket, outgoing_message, validate_session_id},
    state::{OUTBOUND_QUEUE_CAPACITY, SessionControl, WebSocketState},
    transport::{SessionTask, open_socket, run_session},
};
use crate::{
    domain::{
        WebSocketConnectRequest, WebSocketConnectResult, WebSocketDisconnectRequest,
        WebSocketDisconnectResult, WebSocketEventKind, WebSocketSendRequest, WebSocketSendResult,
    },
    error::{AoneError, AoneResult},
    runner::RuntimeState,
};

#[tauri::command]
pub async fn connect_websocket(
    request: WebSocketConnectRequest,
    app: AppHandle,
    state: State<'_, WebSocketState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<WebSocketConnectResult> {
    let prepared = PreparedWebSocket::new(request)?;
    let registry = state.registry();
    let slot = registry.reserve_slot()?;
    let destination = resolve_after_approval(confirm_connection(&app, &prepared), || {
        ValidatedDestination::resolve(&prepared.url)
    })
    .await?;

    let session_id = format!("websocket:{}", Uuid::now_v7());
    let mut secret_values = runtime.secret_values_for_redaction();
    secret_values.extend(prepared.redaction_values());
    let redactor = SecretRedactor::from_values(&secret_values);
    publish(
        &app,
        lifecycle_event(
            &session_id,
            WebSocketEventKind::Connecting,
            &prepared.destination,
            None,
            None,
            &redactor,
        ),
    );
    let (socket, protocol) = match open_socket(&prepared, &destination).await {
        Ok(opened) => opened,
        Err(error) => {
            let detail = error.to_string();
            publish(
                &app,
                lifecycle_event(
                    &session_id,
                    WebSocketEventKind::Error,
                    &prepared.destination,
                    None,
                    Some(&detail),
                    &redactor,
                ),
            );
            publish(
                &app,
                lifecycle_event(
                    &session_id,
                    WebSocketEventKind::Closed,
                    &prepared.destination,
                    None,
                    Some("connection was not opened"),
                    &redactor,
                ),
            );
            return Err(error);
        }
    };

    let (outbound_sender, outbound_receiver) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    registry.register(
        session_id.clone(),
        SessionControl {
            outbound: outbound_sender,
            shutdown: shutdown_sender,
        },
    )?;
    publish(
        &app,
        lifecycle_event(
            &session_id,
            WebSocketEventKind::Open,
            &prepared.destination,
            protocol.clone(),
            None,
            &redactor,
        ),
    );
    tauri::async_runtime::spawn(run_session(
        SessionTask {
            app,
            registry,
            session_id: session_id.clone(),
            destination: prepared.destination,
            protocol: protocol.clone(),
            persistent_secrets: secret_values,
        },
        socket,
        outbound_receiver,
        shutdown_receiver,
        slot,
    ));
    Ok(WebSocketConnectResult {
        session_id,
        protocol,
    })
}

#[tauri::command]
pub async fn send_websocket_message(
    request: WebSocketSendRequest,
    state: State<'_, WebSocketState>,
) -> AoneResult<WebSocketSendResult> {
    let (session_id, message) = outgoing_message(request)?;
    let sender = state
        .registry()
        .outbound_sender(&session_id)
        .ok_or_else(|| AoneError::InvalidRequest("WebSocket session is not open".into()))?;
    sender.try_send(message).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => {
            AoneError::InvalidRequest("WebSocket outbound queue is full".into())
        }
        mpsc::error::TrySendError::Closed(_) => {
            AoneError::InvalidRequest("WebSocket session is no longer open".into())
        }
    })?;
    Ok(WebSocketSendResult { accepted: true })
}

#[tauri::command]
pub async fn disconnect_websocket(
    request: WebSocketDisconnectRequest,
    state: State<'_, WebSocketState>,
) -> AoneResult<WebSocketDisconnectResult> {
    validate_session_id(&request.session_id)?;
    Ok(WebSocketDisconnectResult {
        disconnected: state.registry().disconnect(&request.session_id),
    })
}
