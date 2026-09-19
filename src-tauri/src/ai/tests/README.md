# AI state tests

`configuration.rs` covers hosted secret state and the loopback Ollama readiness
boundary. `provider.rs` covers fixed prompts, consent disclosure, response
extraction, and provider error redaction. `projection.rs` verifies that System
and Neighborhood evidence remain workspace-bound and that read-only
execution-flow queries cannot replace those AI evidence buckets.
