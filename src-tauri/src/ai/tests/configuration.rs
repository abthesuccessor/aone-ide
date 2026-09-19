use std::collections::HashMap;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

use super::super::{
    configuration::configure_ollama_checked,
    provider::verify_ollama_readiness,
    state::{
        AiProviderKind, DEFAULT_ANTHROPIC_MODEL, DEFAULT_OPENAI_MODEL, ProviderTransport,
        SecretState,
    },
};

#[test]
fn defaults_to_cost_sensitive_model_and_requires_backend_key() {
    let state = SecretState::new();
    assert_eq!(state.model(), DEFAULT_OPENAI_MODEL);
    assert!(state.provider_configuration().is_err());

    state
        .replace_env(HashMap::from([(
            "OPENAI_API_KEY".into(),
            "backend-only".into(),
        )]))
        .unwrap();
    let provider = state.provider_configuration().unwrap();
    assert_eq!(provider.kind, AiProviderKind::OpenAi);
    let ProviderTransport::HostedApi { api_key } = provider.transport else {
        panic!("expected hosted API transport");
    };
    assert_eq!(api_key.as_str(), "backend-only");
}

#[test]
fn anthropic_provider_is_selected_from_backend_env() {
    let state = SecretState::new();
    state
        .replace_env(HashMap::from([(
            "ANTHROPIC_API_KEY".into(),
            "anthropic-backend-only".into(),
        )]))
        .unwrap();

    let provider = state.provider_configuration().unwrap();
    assert_eq!(provider.kind, AiProviderKind::Anthropic);
    assert_eq!(provider.model, DEFAULT_ANTHROPIC_MODEL);
    let ProviderTransport::HostedApi { api_key } = provider.transport else {
        panic!("expected hosted API transport");
    };
    assert_eq!(api_key.as_str(), "anthropic-backend-only");
}

#[test]
fn two_provider_keys_require_an_explicit_selection() {
    let state = SecretState::new();
    let error = state
        .replace_env(HashMap::from([
            ("OPENAI_API_KEY".into(), "openai-secret".into()),
            ("ANTHROPIC_API_KEY".into(), "anthropic-secret".into()),
        ]))
        .unwrap_err();
    assert!(error.contains("AONE_AI_PROVIDER"));

    state
        .replace_env(HashMap::from([
            ("AONE_AI_PROVIDER".into(), "anthropic".into()),
            ("OPENAI_API_KEY".into(), "openai-secret".into()),
            ("ANTHROPIC_API_KEY".into(), "anthropic-secret".into()),
        ]))
        .unwrap();
    assert_eq!(state.provider_kind(), AiProviderKind::Anthropic);
}

#[test]
fn openai_model_can_be_overridden_by_backend_env() {
    let state = SecretState::new();
    state
        .replace_env(HashMap::from([(
            "OPENAI_MODEL".into(),
            "gpt-5.6-sol".into(),
        )]))
        .unwrap();
    assert_eq!(state.model(), "gpt-5.6-sol");
}

#[test]
fn ai_configuration_status_is_unconfigured_without_selected_provider_key() {
    let state = SecretState::new();

    let status = state.configuration_status();

    assert!(!status.configured);
    assert_eq!(status.provider, None);
    assert_eq!(status.model, None);
    assert_eq!(status.transport, None);
    assert!(!status.inference_available);
}

#[test]
fn ai_configuration_status_reports_openai_api_inference() {
    let state = SecretState::new();
    state
        .replace_env(HashMap::from([(
            "OPENAI_API_KEY".into(),
            "backend-only-provider-secret".into(),
        )]))
        .unwrap();

    let status = state.configuration_status();

    assert!(status.configured);
    assert_eq!(status.provider.as_deref(), Some("openai"));
    assert_eq!(status.model.as_deref(), Some(DEFAULT_OPENAI_MODEL));
    assert_eq!(status.transport.as_deref(), Some("api"));
    assert!(status.inference_available);
}

#[test]
fn ai_configuration_status_exposes_only_safe_selected_provider_metadata() {
    let state = SecretState::new();
    let provider_key = "backend-only-provider-secret";
    state
        .replace_env(HashMap::from([
            ("AONE_AI_PROVIDER".into(), "anthropic".into()),
            ("ANTHROPIC_API_KEY".into(), provider_key.into()),
            ("ANTHROPIC_MODEL".into(), "claude-sonnet-4-20250514".into()),
        ]))
        .unwrap();

    let status = state.configuration_status();
    assert!(status.configured);
    assert_eq!(status.provider.as_deref(), Some("anthropic"));
    assert_eq!(status.model.as_deref(), Some("claude-sonnet-4-20250514"));
    assert_eq!(status.transport.as_deref(), Some("api"));
    assert!(status.inference_available);

    let serialized = serde_json::to_string(&status).unwrap();
    assert!(serialized.contains("\"inferenceAvailable\":true"));
    assert!(!serialized.contains("inference_available"));
    assert!(!serialized.contains(provider_key));
    assert!(!serialized.contains("API_KEY"));
}

#[test]
fn ai_configuration_status_requires_the_selected_providers_key() {
    let state = SecretState::new();
    state
        .replace_env(HashMap::from([
            ("AONE_AI_PROVIDER".into(), "openai".into()),
            (
                "ANTHROPIC_API_KEY".into(),
                "unselected-provider-secret".into(),
            ),
        ]))
        .unwrap();

    let status = state.configuration_status();
    assert!(!status.configured);
    assert!(!status.inference_available);
    assert_eq!(status.provider, None);
}

#[test]
fn direct_hosted_configuration_is_bounded_and_never_serializes_the_key() {
    let state = SecretState::new();
    let key = "direct-backend-only-secret";
    state
        .configure_hosted(
            "openai".into(),
            Zeroizing::new(key.into()),
            Some("gpt-5.6-sol".into()),
        )
        .unwrap();

    let status = state.configuration_status();
    assert_eq!(status.provider.as_deref(), Some("openai"));
    assert_eq!(status.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(status.transport.as_deref(), Some("api"));
    assert!(!serde_json::to_string(&status).unwrap().contains(key));

    for invalid in [
        Zeroizing::new(String::new()),
        Zeroizing::new("line-one\nline-two".into()),
        Zeroizing::new("x".repeat(16 * 1024 + 1)),
    ] {
        assert!(
            state
                .configure_hosted("openai".into(), invalid, None)
                .is_err()
        );
    }
}

#[test]
fn ollama_configuration_is_normalized_to_fixed_loopback_routes() {
    let prepared =
        SecretState::prepare_ollama("http://localhost:11434/".into(), "qwen2.5-coder:7b".into())
            .unwrap();
    assert_eq!(
        prepared.chat_endpoint.as_str(),
        "http://127.0.0.1:11434/api/chat"
    );
    assert_eq!(
        prepared.tags_endpoint.as_str(),
        "http://127.0.0.1:11434/api/tags"
    );

    for endpoint in [
        "https://127.0.0.1:11434",
        "http://192.168.1.2:11434",
        "http://example.com:11434",
        "http://user:secret@127.0.0.1:11434",
        "http://127.0.0.1:11434/private",
    ] {
        assert!(
            SecretState::prepare_ollama(endpoint.into(), "qwen:latest".into()).is_err(),
            "{endpoint} must be rejected"
        );
    }
}

#[tokio::test]
async fn ollama_readiness_requires_the_requested_installed_model() {
    let (endpoint, request) = serve_once(
        "200 OK",
        &json!({
            "models": [
                {"name": "qwen2.5-coder:7b", "model": "qwen2.5-coder:7b"}
            ]
        })
        .to_string(),
        &[],
    )
    .await;
    let prepared = SecretState::prepare_ollama(endpoint, "qwen2.5-coder:7b".into()).unwrap();

    verify_ollama_readiness(&prepared).await.unwrap();

    assert!(request.await.unwrap().starts_with("GET /api/tags HTTP/1.1"));

    let state = SecretState::new();
    let (endpoint, _) = serve_once(
        "200 OK",
        r#"{"models":[{"model":"qwen2.5-coder:7b"}]}"#,
        &[],
    )
    .await;
    let status = configure_ollama_checked(&state, endpoint, "qwen2.5-coder:7b".into())
        .await
        .unwrap();
    assert_eq!(status.provider.as_deref(), Some("ollama"));
    assert_eq!(status.transport.as_deref(), Some("local_http"));
    assert!(status.inference_available);

    let (endpoint, _) = serve_once("200 OK", r#"{"models":[{"name":"other:latest"}]}"#, &[]).await;
    let missing = SecretState::prepare_ollama(endpoint, "qwen2.5-coder:7b".into()).unwrap();
    assert!(
        verify_ollama_readiness(&missing)
            .await
            .unwrap_err()
            .to_string()
            .contains("not installed")
    );
}

#[tokio::test]
async fn ollama_readiness_does_not_follow_redirects() {
    let (endpoint, request) = serve_once(
        "302 Found",
        "",
        &[("Location", "https://example.com/api/tags")],
    )
    .await;
    let prepared = SecretState::prepare_ollama(endpoint, "qwen:latest".into()).unwrap();

    let error = verify_ollama_readiness(&prepared).await.unwrap_err();

    assert!(error.to_string().contains("HTTP 302"));
    assert!(request.await.unwrap().starts_with("GET /api/tags HTTP/1.1"));
}

#[tokio::test]
async fn failed_ollama_readiness_preserves_the_previous_provider() {
    let state = SecretState::new();
    state
        .configure_hosted(
            "anthropic".into(),
            Zeroizing::new("previous-secret".into()),
            None,
        )
        .unwrap();
    let before = state.configuration_status();
    let (endpoint, _) = serve_once("200 OK", r#"{"models":[]}"#, &[]).await;

    assert!(
        configure_ollama_checked(&state, endpoint, "missing:latest".into())
            .await
            .is_err()
    );

    assert_eq!(state.configuration_status(), before);
}

async fn serve_once(
    status: &'static str,
    body: &str,
    headers: &[(&str, &str)],
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_string();
    let headers = headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 4096];
        let read = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..read]).into_owned();
        let extra = headers
            .into_iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        request
    });
    (format!("http://{address}"), task)
}
