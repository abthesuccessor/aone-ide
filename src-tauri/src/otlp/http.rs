use std::{collections::BTreeMap, io::Read, sync::Arc, time::Duration};

use flate2::read::GzDecoder;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, watch},
};

use super::{ingest::ingest_export, state::OTLP_AUTH_HEADER, wire::ExportTraceServiceRequest};
use crate::runner::RuntimeState;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 8;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn spawn_receiver(
    app: AppHandle,
    listener: TcpListener,
    receiver_id: String,
    workspace_id: String,
    mut shutdown: watch::Receiver<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let connections = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        loop {
            let accepted = tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                    continue;
                }
                accepted = listener.accept() => accepted,
            };
            let (stream, peer) = match accepted {
                Ok(connection) => connection,
                Err(error) => {
                    app.state::<RuntimeState>().otlp.lock().fail(
                        &receiver_id,
                        format!("loopback OTLP listener failed: {error}"),
                    );
                    break;
                }
            };
            if !peer.ip().is_loopback() {
                continue;
            }
            let Ok(permit) = Arc::clone(&connections).try_acquire_owned() else {
                continue;
            };
            let connection_app = app.clone();
            let connection_receiver_id = receiver_id.clone();
            let connection_workspace_id = workspace_id.clone();
            tauri::async_runtime::spawn(async move {
                let _permit = permit;
                let _ = tokio::time::timeout(
                    CONNECTION_TIMEOUT,
                    handle_connection(
                        stream,
                        &connection_app,
                        &connection_receiver_id,
                        &connection_workspace_id,
                    ),
                )
                .await;
            });
        }
    });
}

async fn handle_connection(
    mut stream: TcpStream,
    app: &AppHandle,
    receiver_id: &str,
    workspace_id: &str,
) -> std::io::Result<()> {
    let request = match read_request(&mut stream).await {
        Ok(request) => request,
        Err(response) => return write_response(&mut stream, response).await,
    };
    if request.method != "POST" {
        return write_response(&mut stream, HttpResponse::error(405, "method not allowed")).await;
    }
    if request.path != "/v1/traces" {
        return write_response(
            &mut stream,
            HttpResponse::error(404, "unknown OTLP signal path"),
        )
        .await;
    }
    let content_type = request
        .headers
        .get("content-type")
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some("application/json") {
        return write_response(
            &mut stream,
            HttpResponse::error(415, "only OTLP/HTTP JSON is supported"),
        )
        .await;
    }
    let candidate_token = request
        .headers
        .get(OTLP_AUTH_HEADER)
        .map(String::as_str)
        .unwrap_or_default();
    let runtime = app.state::<RuntimeState>();
    if !runtime
        .otlp
        .lock()
        .authorize(receiver_id, workspace_id, candidate_token)
    {
        return write_response(
            &mut stream,
            HttpResponse::error(401, "invalid ingest token"),
        )
        .await;
    }
    let body = match decode_body(
        &request.body,
        request.headers.get("content-encoding").map(String::as_str),
    ) {
        Ok(body) => body,
        Err(response) => return write_response(&mut stream, response).await,
    };
    let export = match serde_json::from_slice::<ExportTraceServiceRequest>(&body) {
        Ok(export) => export,
        Err(_) => {
            runtime.otlp.lock().record_rejected(receiver_id, 1);
            return write_response(&mut stream, HttpResponse::error(400, "invalid OTLP JSON"))
                .await;
        }
    };
    let result = match ingest_export(app, receiver_id, workspace_id, export) {
        Ok(result) => result,
        Err(_) => {
            return write_response(
                &mut stream,
                HttpResponse::error(409, "receiver workspace changed or stopped"),
            )
            .await;
        }
    };
    let body = if result.rejected == 0 {
        b"{}".to_vec()
    } else {
        serde_json::to_vec(&json!({
            "partialSuccess": {
                "rejectedSpans": result.rejected.to_string(),
                "errorMessage": format!(
                    "Aone accepted {} bounded spans; invalid, duplicate, or over-capacity spans were rejected",
                    result.accepted,
                ),
            }
        }))
        .unwrap_or_else(|_| b"{}".to_vec())
    };
    write_response(&mut stream, HttpResponse { status: 200, body }).await
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl HttpResponse {
    fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&json!({ "error": message }))
                .unwrap_or_else(|_| b"{}".to_vec()),
        }
    }
}

async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, HttpResponse> {
    let mut bytes = Vec::with_capacity(4_096);
    let header_end = loop {
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err(HttpResponse::error(431, "request headers are too large"));
        }
        let mut buffer = [0_u8; 2_048];
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| HttpResponse::error(400, "could not read request"))?;
        if read == 0 {
            return Err(HttpResponse::error(400, "incomplete request headers"));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let header_text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| HttpResponse::error(400, "request headers must be UTF-8"))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpResponse::error(400, "missing request line"))?;
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(HttpResponse::error(400, "only HTTP/1.1 is supported"));
    }
    let method = parts[0].to_owned();
    let path = parts[1].to_owned();
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| HttpResponse::error(400, "malformed request header"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name.is_empty()
            || value.chars().any(char::is_control)
            || headers.insert(name, value).is_some()
        {
            return Err(HttpResponse::error(
                400,
                "invalid or duplicate request header",
            ));
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(HttpResponse::error(
            400,
            "chunked requests are not supported",
        ));
    }
    let content_length = headers
        .get("content-length")
        .ok_or_else(|| HttpResponse::error(411, "content-length is required"))?
        .parse::<usize>()
        .map_err(|_| HttpResponse::error(400, "invalid content-length"))?;
    if content_length == 0 || content_length > MAX_BODY_BYTES {
        return Err(HttpResponse::error(
            413,
            "OTLP request body exceeds the bound",
        ));
    }
    let body_start = header_end + 4;
    let total_length = body_start
        .checked_add(content_length)
        .ok_or_else(|| HttpResponse::error(413, "OTLP request body exceeds the bound"))?;
    while bytes.len() < total_length {
        let mut buffer = [0_u8; 8_192];
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| HttpResponse::error(400, "could not read request body"))?;
        if read == 0 {
            return Err(HttpResponse::error(400, "incomplete request body"));
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > total_length {
            return Err(HttpResponse::error(
                400,
                "request exceeded declared content-length",
            ));
        }
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: bytes[body_start..total_length].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn decode_body(body: &[u8], encoding: Option<&str>) -> Result<Vec<u8>, HttpResponse> {
    match encoding
        .unwrap_or("identity")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "identity" => Ok(body.to_vec()),
        "gzip" => {
            let mut decoder = GzDecoder::new(body).take((MAX_BODY_BYTES + 1) as u64);
            let mut decoded = Vec::new();
            decoder
                .read_to_end(&mut decoded)
                .map_err(|_| HttpResponse::error(400, "invalid gzip request body"))?;
            if decoded.len() > MAX_BODY_BYTES {
                return Err(HttpResponse::error(
                    413,
                    "decompressed OTLP body exceeds the bound",
                ));
            }
            Ok(decoded)
        }
        _ => Err(HttpResponse::error(415, "unsupported content encoding")),
    }
}

async fn write_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    };
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.body.len(),
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(&response.body).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{Compression, write::GzEncoder};

    use super::*;

    #[test]
    fn gzip_is_bounded_and_round_trips() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(br#"{"resourceSpans":[]}"#).unwrap();
        let encoded = encoder.finish().unwrap();
        assert_eq!(
            decode_body(&encoded, Some("gzip")).unwrap(),
            br#"{"resourceSpans":[]}"#
        );
        assert!(decode_body(b"not-gzip", Some("gzip")).is_err());
    }
}
