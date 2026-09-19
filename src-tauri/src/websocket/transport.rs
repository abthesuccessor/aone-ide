use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Manager};
use tokio::{
    net::TcpStream,
    sync::{OwnedSemaphorePermit, mpsc, watch},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, client_async_tls_with_config,
    tungstenite::{
        Error as WebSocketError, Message, http::header::SEC_WEBSOCKET_PROTOCOL,
        protocol::WebSocketConfig,
    },
};
use zeroize::Zeroizing;

use super::{
    destination::ValidatedDestination,
    events::{dropped_event, lifecycle_event, message_event, publish},
    rate_limit::InboundRateLimiter,
    redaction::{RedactionCache, SecretRedactor},
    request::PreparedWebSocket,
    state::SessionRegistry,
};
use crate::{
    domain::WebSocketEventKind,
    error::{AoneError, AoneResult},
    runner::RuntimeState,
};

const IO_TIMEOUT_SECONDS: u64 = 10;
const BUFFER_BYTES: usize = 16 * 1024;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_WRITE_BUFFER_BYTES: usize = MAX_MESSAGE_BYTES + BUFFER_BYTES + 1;

pub(super) type ClientSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(super) struct SessionTask {
    pub(super) app: AppHandle,
    pub(super) registry: Arc<SessionRegistry>,
    pub(super) session_id: String,
    pub(super) destination: String,
    pub(super) protocol: Option<String>,
    pub(super) persistent_secrets: Vec<Zeroizing<String>>,
}

pub(super) async fn open_socket(
    request: &PreparedWebSocket,
    destination: &ValidatedDestination,
) -> AoneResult<(ClientSocket, Option<String>)> {
    let handshake = request.handshake_request()?;
    let config = WebSocketConfig::default()
        .read_buffer_size(BUFFER_BYTES)
        .write_buffer_size(BUFFER_BYTES)
        .max_write_buffer_size(MAX_WRITE_BUFFER_BYTES)
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES))
        .accept_unmasked_frames(false);
    let opened = tokio::time::timeout(Duration::from_millis(request.timeout_ms), async move {
        let stream = connect_direct(&destination.addresses)
            .await
            .map_err(OpenFailure::Tcp)?;
        client_async_tls_with_config(handshake, stream, Some(config), None)
            .await
            .map_err(OpenFailure::Handshake)
    })
    .await
    .map_err(|_| AoneError::Task("WebSocket connection timed out".into()))?
    .map_err(|failure| AoneError::Task(failure.safe_message()))?;
    let (socket, response) = opened;
    let protocol = response
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    Ok((socket, protocol))
}

pub(super) async fn run_session(
    task: SessionTask,
    mut socket: ClientSocket,
    mut outbound: mpsc::Receiver<Message>,
    mut shutdown: watch::Receiver<bool>,
    _slot: OwnedSemaphorePermit,
) {
    let SessionTask {
        app,
        registry,
        session_id,
        destination,
        protocol,
        persistent_secrets,
    } = task;
    let mut redaction_cache = RedactionCache::new(persistent_secrets);
    let mut inbound_limiter = InboundRateLimiter::new();
    let close_detail = loop {
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => {
                        let _ = tokio::time::timeout(
                            Duration::from_secs(IO_TIMEOUT_SECONDS),
                            socket.send(Message::Close(None)),
                        ).await;
                        break "closed by user".into();
                    }
                    Ok(()) => {}
                    Err(_) => break "session shutdown signal closed".into(),
                }
            }
            outgoing = outbound.recv() => {
                let Some(message) = outgoing else {
                    break "session command channel closed".into();
                };
                match tokio::time::timeout(
                    Duration::from_secs(IO_TIMEOUT_SECONDS),
                    socket.send(message),
                ).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        let detail = runtime_error_message(&error);
                        let redactor = current_redactor(&app, &mut redaction_cache);
                        publish(&app, lifecycle_event(
                            &session_id,
                            WebSocketEventKind::Error,
                            &destination,
                            protocol.clone(),
                            Some(detail),
                            redactor,
                        ));
                        break detail.into();
                    }
                    Err(_) => {
                        let detail = "WebSocket send timed out";
                        let redactor = current_redactor(&app, &mut redaction_cache);
                        publish(&app, lifecycle_event(
                            &session_id,
                            WebSocketEventKind::Error,
                            &destination,
                            protocol.clone(),
                            Some(detail),
                            redactor,
                        ));
                        break detail.into();
                    }
                }
            }
            incoming = socket.next() => {
                match incoming {
                    Some(Ok(Message::Close(frame))) => {
                        let detail = frame.map_or_else(
                            || "remote endpoint closed the connection".into(),
                            |frame| format!(
                                "remote close code {}: {}",
                                u16::from(frame.code),
                                frame.reason,
                            ),
                        );
                        let _ = tokio::time::timeout(
                            Duration::from_secs(IO_TIMEOUT_SECONDS),
                            socket.flush(),
                        ).await;
                        break detail;
                    }
                    Some(Ok(message @ (Message::Text(_) | Message::Binary(_)))) => {
                        let admission = inbound_limiter.admit();
                        if admission.dropped_before > 0 {
                            publish(&app, dropped_event(
                                &session_id,
                                &destination,
                                protocol.clone(),
                                admission.dropped_before,
                            ));
                        }
                        if admission.allow_message {
                            let redactor = current_redactor(&app, &mut redaction_cache);
                            if let Some(event) = message_event(
                                &session_id,
                                &destination,
                                protocol.clone(),
                                &message,
                                redactor,
                            ) {
                                publish(&app, event);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(_))) => {
                        match tokio::time::timeout(
                            Duration::from_secs(IO_TIMEOUT_SECONDS),
                            socket.flush(),
                        ).await {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                let detail = runtime_error_message(&error);
                                let redactor = current_redactor(&app, &mut redaction_cache);
                                publish(&app, lifecycle_event(
                                    &session_id,
                                    WebSocketEventKind::Error,
                                    &destination,
                                    protocol.clone(),
                                    Some(detail),
                                    redactor,
                                ));
                                break detail.into();
                            }
                            Err(_) => break "WebSocket pong timed out".into(),
                        }
                    }
                    Some(Ok(Message::Pong(_) | Message::Frame(_))) => {}
                    Some(Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed)) => {
                        break "remote endpoint closed the connection".into();
                    }
                    Some(Err(error)) => {
                        let detail = runtime_error_message(&error);
                        let redactor = current_redactor(&app, &mut redaction_cache);
                        publish(&app, lifecycle_event(
                            &session_id,
                            WebSocketEventKind::Error,
                            &destination,
                            protocol.clone(),
                            Some(detail),
                            redactor,
                        ));
                        break detail.into();
                    }
                    None => {
                        break "remote endpoint ended the transport".into();
                    }
                }
            }
        }
    };
    registry.finish(&session_id);
    let dropped = inbound_limiter.take_dropped();
    if dropped > 0 {
        publish(
            &app,
            dropped_event(&session_id, &destination, protocol.clone(), dropped),
        );
    }
    let redactor = current_redactor(&app, &mut redaction_cache);
    publish(
        &app,
        lifecycle_event(
            &session_id,
            WebSocketEventKind::Closed,
            &destination,
            protocol,
            Some(&close_detail),
            redactor,
        ),
    );
}

fn current_redactor<'a>(app: &AppHandle, cache: &'a mut RedactionCache) -> &'a SecretRedactor {
    let dynamic = app
        .try_state::<RuntimeState>()
        .map(|runtime| runtime.secret_values_for_redaction())
        .unwrap_or_default();
    cache.refresh(&dynamic)
}

async fn connect_direct(addresses: &[SocketAddr]) -> io::Result<TcpStream> {
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect(address).await {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no validated address")))
}

enum OpenFailure {
    Tcp(io::Error),
    Handshake(WebSocketError),
}

impl OpenFailure {
    fn safe_message(&self) -> String {
        match self {
            Self::Tcp(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                "WebSocket destination refused the connection".into()
            }
            Self::Tcp(_) => "could not connect to the validated WebSocket destination".into(),
            Self::Handshake(WebSocketError::Http(response))
                if response.status().is_redirection() =>
            {
                "WebSocket handshake returned a redirect; redirects are disabled".into()
            }
            Self::Handshake(WebSocketError::Http(response)) => {
                format!(
                    "WebSocket handshake failed with HTTP status {}",
                    response.status()
                )
            }
            Self::Handshake(WebSocketError::Tls(_)) => "WebSocket TLS verification failed".into(),
            Self::Handshake(_) => "WebSocket handshake failed".into(),
        }
    }
}

fn runtime_error_message(error: &WebSocketError) -> &'static str {
    match error {
        WebSocketError::Capacity(_) | WebSocketError::WriteBufferFull(_) => {
            "WebSocket size or buffer limit was exceeded"
        }
        WebSocketError::Protocol(_) | WebSocketError::AttackAttempt => {
            "remote endpoint violated the WebSocket protocol"
        }
        WebSocketError::Utf8(_) => "remote endpoint sent invalid UTF-8",
        WebSocketError::Tls(_) => "WebSocket TLS transport failed",
        WebSocketError::Io(_) => "WebSocket network transport failed",
        _ => "WebSocket transport failed",
    }
}
