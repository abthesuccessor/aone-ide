# AsyncAPI validation

`check.mjs` parses the dependency-free JSON AsyncAPI document, requires the
stable 3.1.0 version, verifies one backend-send operation per channel, and checks
that documented channel addresses exactly match the Tauri listeners in the
renderer bridge and exist in the Rust emitter source.
