use reqwest::StatusCode;
use serde_json::json;
use zeroize::Zeroizing;

use super::super::{
    provider::{
        ENGINEER_FLOW_QUESTION, MAX_OUTPUT_TOKENS, PROJECT_ENVIRONMENT_QUESTION,
        extract_anthropic_text, extract_ollama_text, extract_output_text,
        project_environment_confirmation_message, project_environment_responses_request,
        provider_confirmation_message, provider_request, provider_status_error, responses_request,
    },
    state::AiProviderKind,
};

#[test]
fn responses_request_uses_only_rust_owned_question_and_bounds_output() {
    let body = responses_request("gpt-5.6-luna", "[node:1] fact");
    assert_eq!(body["store"], false);
    assert_eq!(body["max_output_tokens"], MAX_OUTPUT_TOKENS);
    assert_eq!(body["model"], "gpt-5.6-luna");
    let input = body["input"].as_str().unwrap();
    assert!(input.contains(ENGINEER_FLOW_QUESTION));
    assert!(input.contains("[node:1]"));
}

#[test]
fn provider_confirmation_discloses_only_model_count_and_fixed_task() {
    let message = provider_confirmation_message(AiProviderKind::OpenAi, "gpt-5.6-luna", 3);
    assert!(message.contains("Model: gpt-5.6-luna"));
    assert!(message.contains("Evidence items: 3"));
    assert!(message.contains(ENGINEER_FLOW_QUESTION));
    assert!(!message.contains("[node:"));
    assert!(!message.contains("OPENAI_API_KEY"));
}

#[test]
fn project_environment_provider_task_is_fixed_and_consent_is_specific() {
    let body = project_environment_responses_request(
        "gpt-5.6-luna",
        "[environment:tool:cargo] observed tool status",
    );
    let input = body["input"].as_str().unwrap();
    assert!(input.contains(PROJECT_ENVIRONMENT_QUESTION));
    assert!(input.contains("[environment:tool:cargo]"));
    assert!(!input.contains(ENGINEER_FLOW_QUESTION));

    let message =
        project_environment_confirmation_message(AiProviderKind::OpenAi, "gpt-5.6-luna", 4, 512);
    assert!(message.contains("https://api.openai.com"));
    assert!(message.contains("Evidence items: 4"));
    assert!(message.contains("Evidence size: 512 bytes"));
    assert!(message.contains(PROJECT_ENVIRONMENT_QUESTION));
    assert!(!message.contains("/Users/"));
}

#[test]
fn anthropic_request_and_consent_use_the_fixed_messages_api_shape() {
    let body = provider_request(
        AiProviderKind::Anthropic,
        "claude-sonnet-4-20250514",
        "[node:1] fact",
    );
    assert_eq!(body["model"], "claude-sonnet-4-20250514");
    assert_eq!(body["max_tokens"], MAX_OUTPUT_TOKENS);
    assert!(body.get("store").is_none());
    assert!(body["system"].as_str().unwrap().contains("untrusted data"));
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains(ENGINEER_FLOW_QUESTION)
    );

    let message =
        provider_confirmation_message(AiProviderKind::Anthropic, "claude-sonnet-4-20250514", 2);
    assert!(message.contains("Provider: Anthropic"));
    assert!(message.contains("https://api.anthropic.com"));
    assert!(!message.contains("ANTHROPIC_API_KEY"));
}

#[test]
fn ollama_request_and_consent_use_the_fixed_chat_shape() {
    let body = provider_request(AiProviderKind::Ollama, "qwen2.5-coder:7b", "[node:1] fact");
    assert_eq!(body["model"], "qwen2.5-coder:7b");
    assert_eq!(body["stream"], false);
    assert_eq!(body["options"]["num_predict"], MAX_OUTPUT_TOKENS);
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("untrusted data")
    );

    let message = provider_confirmation_message(AiProviderKind::Ollama, "qwen2.5-coder:7b", 2);
    assert!(message.contains("Provider: Ollama"));
    assert!(message.contains("http://127.0.0.1:11434/api/chat"));
}

#[test]
fn extracts_all_output_text_parts() {
    let payload = json!({
        "output": [{
            "type": "message",
            "content": [
                {"type": "output_text", "text": "first"},
                {"type": "refusal", "refusal": "ignored"},
                {"type": "output_text", "text": "second"}
            ]
        }]
    });
    assert_eq!(extract_output_text(&payload).unwrap(), "first\nsecond");
}

#[test]
fn extracts_all_anthropic_text_parts() {
    let payload = json!({
        "content": [
            {"type": "text", "text": "first"},
            {"type": "tool_use", "name": "ignored"},
            {"type": "text", "text": "second"}
        ]
    });
    assert_eq!(extract_anthropic_text(&payload).unwrap(), "first\nsecond");
}

#[test]
fn extracts_ollama_chat_message_content() {
    let payload = json!({"message": {"role": "assistant", "content": "local result"}});
    assert_eq!(
        extract_ollama_text(&payload).as_deref(),
        Some("local result")
    );
}

#[test]
fn anthropic_error_details_redact_provider_credentials() {
    let secret = Zeroizing::new("anthropic-backend-only".to_string());
    let payload = json!({
        "error": {"message": "invalid key anthropic-backend-only"}
    });
    let error = provider_status_error(
        AiProviderKind::Anthropic,
        StatusCode::UNAUTHORIZED,
        &payload,
        &[secret],
    );
    let message = error.to_string();
    assert!(message.contains("Anthropic request failed"));
    assert!(message.contains("[REDACTED]"));
    assert!(!message.contains("anthropic-backend-only"));
}
