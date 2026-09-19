use std::time::Duration;

use reqwest::{Client, StatusCode, redirect::Policy};
use serde_json::Value;
use zeroize::Zeroizing;

use super::super::{
    evidence::bounded_text,
    state::{AiProviderKind, PreparedOllamaConfiguration},
};
use crate::{
    error::{AoneError, AoneResult},
    runner::redact_text,
};

const OLLAMA_READINESS_TIMEOUT_SECONDS: u64 = 5;
const MAX_PROVIDER_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_OLLAMA_TAGS_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_PROVIDER_ERROR_BYTES: usize = 800;

pub(in crate::ai) fn extract_output_text(payload: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(output) = payload.get("output").and_then(Value::as_array) {
        for item in output {
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                continue;
            };
            for part in content {
                if part.get("type").and_then(Value::as_str) == Some("output_text")
                    && let Some(text) = part.get("text").and_then(Value::as_str)
                {
                    parts.push(text);
                }
            }
        }
    }
    if parts.is_empty()
        && let Some(text) = payload.get("output_text").and_then(Value::as_str)
    {
        parts.push(text);
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

pub(in crate::ai) fn extract_anthropic_text(payload: &Value) -> Option<String> {
    let parts = payload
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

pub(in crate::ai) fn extract_ollama_text(payload: &Value) -> Option<String> {
    payload
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(in crate::ai) async fn verify_ollama_readiness(
    configuration: &PreparedOllamaConfiguration,
) -> AoneResult<()> {
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .map_err(|_| AoneError::Task("could not initialize the Ollama readiness client".into()))?;
    let response = client
        .get(configuration.tags_endpoint.clone())
        .timeout(Duration::from_secs(OLLAMA_READINESS_TIMEOUT_SECONDS))
        .send()
        .await
        .map_err(|error| {
            AoneError::Task(classify_provider_error(AiProviderKind::Ollama, &error))
        })?;
    if !response.status().is_success() {
        return Err(AoneError::Task(format!(
            "Ollama readiness check failed with HTTP {}",
            response.status()
        )));
    }
    let (bytes, truncated) = read_response_with_limit(
        response,
        AiProviderKind::Ollama,
        MAX_OLLAMA_TAGS_RESPONSE_BYTES,
    )
    .await?;
    if truncated {
        return Err(AoneError::Task(
            "Ollama model list exceeded the local safety limit".into(),
        ));
    }
    let payload: Value = serde_json::from_slice(&bytes)
        .map_err(|_| AoneError::Task("Ollama returned an invalid model list".into()))?;
    let matched = payload
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|entry| {
            ["name", "model"].iter().any(|field| {
                entry.get(field).and_then(Value::as_str) == Some(configuration.model.as_str())
            })
        });
    if !matched {
        return Err(AoneError::InvalidRequest(
            "the requested Ollama model is not installed at this endpoint".into(),
        ));
    }
    Ok(())
}

pub(super) async fn read_provider_response_bounded(
    response: reqwest::Response,
    provider: AiProviderKind,
) -> AoneResult<(Vec<u8>, bool)> {
    read_response_with_limit(response, provider, MAX_PROVIDER_RESPONSE_BYTES).await
}

async fn read_response_with_limit(
    mut response: reqwest::Response,
    provider: AiProviderKind,
    limit: usize,
) -> AoneResult<(Vec<u8>, bool)> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AoneError::Task(format!("failed to read {} response", provider.label())))?
    {
        let remaining = limit.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            return Ok((body, true));
        }
        body.extend_from_slice(&chunk);
        if body.len() == limit {
            return Ok((body, true));
        }
    }
    Ok((body, false))
}

pub(in crate::ai) fn provider_status_error(
    provider: AiProviderKind,
    status: StatusCode,
    payload: &Value,
    redaction_values: &[Zeroizing<String>],
) -> AoneError {
    let message = payload
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| payload.get("error").and_then(Value::as_str))
        .map(|message| redact_text(message, redaction_values))
        .map(|message| bounded_text(&message, MAX_PROVIDER_ERROR_BYTES))
        .unwrap_or_else(|| "provider returned no safe error details".into());
    AoneError::Task(format!(
        "{} request failed with HTTP {status}: {message}",
        provider.label()
    ))
}

pub(super) fn classify_provider_error(provider: AiProviderKind, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("{} request timed out", provider.label())
    } else if error.is_connect() {
        format!("could not connect to {}", provider.label())
    } else {
        format!("{} request failed", provider.label())
    }
}
