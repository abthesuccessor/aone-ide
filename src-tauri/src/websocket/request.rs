use std::{collections::BTreeSet, fmt, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::Url;
use tokio_tungstenite::tungstenite::{
    Message,
    client::IntoClientRequest,
    http::{HeaderMap, HeaderName, HeaderValue, Request, header::SEC_WEBSOCKET_PROTOCOL},
};
use zeroize::Zeroizing;

use crate::{
    domain::{WebSocketConnectRequest, WebSocketPayloadEncoding, WebSocketSendRequest},
    error::{AoneError, AoneResult},
};

pub(super) const MAX_URL_BYTES: usize = 16 * 1024;
pub(super) const MAX_HEADERS: usize = 64;
pub(super) const MAX_HEADER_BYTES: usize = 32 * 1024;
pub(super) const MAX_PROTOCOLS: usize = 16;
pub(super) const MAX_PROTOCOL_BYTES: usize = 1_024;
pub(super) const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_ENCODED_MESSAGE_BYTES: usize = 1_398_104;
const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const MIN_TIMEOUT_MS: u64 = 250;
const MAX_TIMEOUT_MS: u64 = 60_000;

const MANAGED_HEADERS: [&str; 7] = [
    "connection",
    "host",
    "sec-websocket-extensions",
    "sec-websocket-key",
    "sec-websocket-protocol",
    "sec-websocket-version",
    "upgrade",
];

pub(super) struct PreparedWebSocket {
    pub(super) url: Url,
    pub(super) headers: HeaderMap,
    pub(super) header_names: Vec<String>,
    pub(super) protocols: Vec<String>,
    pub(super) timeout_ms: u64,
    pub(super) destination: String,
    redaction_values: Vec<Zeroizing<String>>,
}

impl PreparedWebSocket {
    pub(super) fn new(input: WebSocketConnectRequest) -> AoneResult<Self> {
        if input.url.len() > MAX_URL_BYTES {
            return Err(invalid(format!(
                "WebSocket URL exceeds {MAX_URL_BYTES} bytes"
            )));
        }
        let url =
            Url::parse(input.url.trim()).map_err(|_| invalid("invalid WebSocket URL".into()))?;
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(invalid("only ws and wss URLs are supported".into()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(invalid(
                "credentials in WebSocket URLs are not allowed; use a header".into(),
            ));
        }
        if url.host_str().is_none() {
            return Err(invalid("WebSocket URL must include a host".into()));
        }
        if url.fragment().is_some() {
            return Err(invalid("WebSocket URL fragments are not supported".into()));
        }
        if url.as_str().len() > MAX_URL_BYTES {
            return Err(invalid(format!(
                "canonical WebSocket URL exceeds {MAX_URL_BYTES} bytes"
            )));
        }

        let (headers, header_names, mut redaction_values) = validate_headers(input.headers)?;
        redaction_values.extend(
            url.query_pairs()
                .map(|(_, value)| value.into_owned())
                .filter(|value| !value.is_empty())
                .map(Zeroizing::new),
        );
        let protocols = validate_protocols(input.protocols)?;
        let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
            return Err(invalid(format!(
                "WebSocket timeout must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS} milliseconds"
            )));
        }
        let destination = url.origin().ascii_serialization();
        Ok(Self {
            url,
            headers,
            header_names,
            protocols,
            timeout_ms,
            destination,
            redaction_values,
        })
    }

    pub(super) fn handshake_request(&self) -> AoneResult<Request<()>> {
        let mut request = self
            .url
            .as_str()
            .into_client_request()
            .map_err(|_| invalid("could not construct WebSocket handshake".into()))?;
        for (name, value) in &self.headers {
            request.headers_mut().append(name, value.clone());
        }
        if !self.protocols.is_empty() {
            let value = HeaderValue::from_str(&self.protocols.join(", "))
                .map_err(|_| invalid("invalid WebSocket subprotocol list".into()))?;
            request.headers_mut().insert(SEC_WEBSOCKET_PROTOCOL, value);
        }
        Ok(request)
    }

    pub(super) fn redaction_values(&self) -> Vec<Zeroizing<String>> {
        self.redaction_values.clone()
    }

    pub(super) fn consent_destination(&self) -> String {
        let host = self.url.host_str().unwrap_or_default();
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        let port = self.url.port_or_known_default().unwrap_or_default();
        let mut destination = format!(
            "{}://{}:{}{}",
            self.url.scheme(),
            host,
            port,
            self.url.path()
        );
        let names = self
            .url
            .query_pairs()
            .map(|(name, _)| name.into_owned())
            .collect::<BTreeSet<_>>();
        if !names.is_empty() {
            let mut sanitized = self.url.clone();
            sanitized.set_query(None);
            sanitized
                .query_pairs_mut()
                .extend_pairs(names.iter().map(|name| (name.as_str(), "[value hidden]")));
            if let Some(query) = sanitized.query() {
                destination.push('?');
                destination.push_str(query);
            }
        }
        destination
    }
}

impl fmt::Debug for PreparedWebSocket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedWebSocket")
            .field("destination", &self.destination)
            .field("header_names", &self.header_names)
            .field("protocol_count", &self.protocols.len())
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

pub(super) fn outgoing_message(input: WebSocketSendRequest) -> AoneResult<(String, Message)> {
    validate_session_id(&input.session_id)?;
    if input.data.len() > MAX_ENCODED_MESSAGE_BYTES {
        return Err(invalid("outbound WebSocket message is too large".into()));
    }
    let message = match input.encoding {
        WebSocketPayloadEncoding::Text => {
            if input.data.len() > MAX_MESSAGE_BYTES {
                return Err(invalid(format!(
                    "outbound WebSocket message exceeds {MAX_MESSAGE_BYTES} bytes"
                )));
            }
            Message::text(input.data)
        }
        WebSocketPayloadEncoding::Base64 => {
            let decoded = STANDARD
                .decode(input.data.as_bytes())
                .map_err(|_| invalid("binary WebSocket data must be valid base64".into()))?;
            if decoded.len() > MAX_MESSAGE_BYTES {
                return Err(invalid(format!(
                    "decoded WebSocket message exceeds {MAX_MESSAGE_BYTES} bytes"
                )));
            }
            Message::binary(decoded)
        }
    };
    Ok((input.session_id, message))
}

pub(super) fn validate_session_id(value: &str) -> AoneResult<()> {
    let uuid = value
        .strip_prefix("websocket:")
        .ok_or_else(|| invalid("invalid WebSocket session id".into()))?;
    if value.len() > 64 || uuid::Uuid::parse_str(uuid).is_err() {
        return Err(invalid("invalid WebSocket session id".into()));
    }
    Ok(())
}

fn validate_headers(
    input: Vec<crate::domain::ApiHeader>,
) -> AoneResult<(HeaderMap, Vec<String>, Vec<Zeroizing<String>>)> {
    if input.len() > MAX_HEADERS {
        return Err(invalid(format!(
            "too many WebSocket headers; maximum is {MAX_HEADERS}"
        )));
    }
    let mut headers = HeaderMap::new();
    let mut names = BTreeSet::new();
    let mut redaction_values = Vec::with_capacity(input.len());
    let mut total = 0_usize;
    for header in input {
        total = total
            .checked_add(header.name.len())
            .and_then(|size| size.checked_add(header.value.len()))
            .ok_or_else(|| invalid("WebSocket headers are too large".into()))?;
        if total > MAX_HEADER_BYTES {
            return Err(invalid(format!(
                "WebSocket headers exceed {MAX_HEADER_BYTES} aggregate bytes"
            )));
        }
        let name = HeaderName::from_str(header.name.trim())
            .map_err(|_| invalid("invalid WebSocket header name".into()))?;
        let normalized = name.as_str().to_owned();
        if MANAGED_HEADERS.binary_search(&normalized.as_str()).is_ok() {
            return Err(invalid(format!(
                "header {normalized} is managed by the WebSocket client"
            )));
        }
        if !names.insert(normalized.clone()) {
            return Err(invalid(format!(
                "duplicate WebSocket header {normalized} is not supported"
            )));
        }
        let mut value = HeaderValue::from_str(&header.value)
            .map_err(|_| invalid("invalid WebSocket header value".into()))?;
        let redaction_value = value
            .to_str()
            .map_err(|_| invalid("WebSocket header values must be visible ASCII".into()))?;
        if !redaction_value.is_empty() {
            redaction_values.push(Zeroizing::new(redaction_value.to_owned()));
        }
        // Prevent accidental disclosure if a future diagnostic formats the
        // prepared HeaderMap. The value is still serialized on the wire.
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    Ok((headers, names.into_iter().collect(), redaction_values))
}

fn validate_protocols(input: Vec<String>) -> AoneResult<Vec<String>> {
    if input.len() > MAX_PROTOCOLS {
        return Err(invalid(format!(
            "too many WebSocket subprotocols; maximum is {MAX_PROTOCOLS}"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut protocols = Vec::with_capacity(input.len());
    let mut total = 0_usize;
    for protocol in input {
        let protocol = protocol.trim();
        total = total
            .checked_add(protocol.len())
            .ok_or_else(|| invalid("WebSocket subprotocols are too large".into()))?;
        if protocol.is_empty() || protocol.len() > 128 || !protocol.bytes().all(is_http_token_byte)
        {
            return Err(invalid("invalid WebSocket subprotocol".into()));
        }
        if total > MAX_PROTOCOL_BYTES {
            return Err(invalid(format!(
                "WebSocket subprotocols exceed {MAX_PROTOCOL_BYTES} aggregate bytes"
            )));
        }
        if !seen.insert(protocol.to_owned()) {
            return Err(invalid("duplicate WebSocket subprotocol".into()));
        }
        protocols.push(protocol.to_owned());
    }
    Ok(protocols)
}

fn is_http_token_byte(value: u8) -> bool {
    value.is_ascii_alphanumeric()
        || matches!(
            value,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn invalid(message: String) -> AoneError {
    AoneError::InvalidRequest(message)
}
