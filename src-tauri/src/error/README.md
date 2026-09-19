# Error boundary

The error feature defines the single `AoneError` taxonomy and `AoneResult` alias used across the
backend. `types.rs` keeps filesystem, database, parser, watcher, validation, and task failures in one
place while `mod.rs` preserves the existing `crate::error::{AoneError, AoneResult}` path.

## Data flow and security

Internal errors are converted to stable, human-readable strings at the Tauri serialization boundary.
Validation failures remain distinguishable from I/O and database failures, including explicit path
escape, sensitive-path, binary-file, and file-size cases. The type contains no secret-bearing debug
payload beyond the messages supplied by callers; credential values must continue to be redacted by
their owning subsystem before an error is constructed.

## Tests

Every Rust feature test exercises this shared result type through its normal failure paths. Strict
Clippy and whole-crate tests verify all conversion implementations and public re-exports compile.
