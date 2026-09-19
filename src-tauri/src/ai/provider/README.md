# AI provider response boundary

`response.rs` owns bounded hosted and local HTTP response reads, provider-safe
error redaction, output extraction, and the loopback Ollama readiness check.
The parent provider module retains prompt construction, native consent, and
transport dispatch, including the existing Codex CLI adapter path.
