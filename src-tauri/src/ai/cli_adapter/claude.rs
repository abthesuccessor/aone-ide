//! Claude Code specifics: the minimum audited version, the argument surface
//! that switches off every ambient capability, and the result envelope.

use std::ffi::OsString;

use serde::Deserialize;

use super::{
    CliAdapterError, CliKind, JSON_SCHEMA, StructuredAnswer, bounded_answer, first_version_line,
};

/// Claude Code is accepted from a minimum audited version rather than one exact
/// build. Its isolation surface (`--tools ""`, `--strict-mcp-config`,
/// `--disable-slash-commands`, `--setting-sources ""`) is stable published CLI
/// contract, unlike the Codex `permissions.*` config keys, which are pinned.
pub(super) const MINIMUM_VERSION: (u64, u64, u64) = (2, 1, 0);

/// Hard ceiling on spend for a single inference. Aone sends one bounded prompt
/// with every tool disabled, so a normal call is far below this.
const MAX_BUDGET_USD: &str = "0.50";

/// The bounded subset of `claude --output-format json` Aone relies on. Claude
/// Code emits many more fields; unknown ones are ignored on purpose so a CLI
/// upgrade that adds telemetry cannot break a local install.
#[derive(Deserialize)]
struct ResultEnvelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AuthStatus {
    #[serde(rename = "loggedIn", default)]
    pub(super) logged_in: bool,
}

pub(super) fn authentication_arguments() -> Vec<OsString> {
    vec![
        OsString::from("auth"),
        OsString::from("status"),
        OsString::from("--json"),
    ]
}

/// The fixed Claude Code surface. Every capability that could read the machine,
/// reach the network through a tool, execute a command, or persist state is
/// switched off explicitly rather than left at its default: no built-in tools,
/// no skills, no MCP servers, no settings files, no session on disk, and a hard
/// spend ceiling. The prompt arrives on stdin.
pub(super) fn execution_arguments() -> Vec<OsString> {
    vec![
        "--print".into(),
        "--output-format".into(),
        "json".into(),
        "--json-schema".into(),
        OsString::from(String::from_utf8_lossy(JSON_SCHEMA).into_owned()),
        // "" is the documented value that disables the entire built-in tool set.
        "--tools".into(),
        "".into(),
        "--disable-slash-commands".into(),
        // With no --mcp-config, strict mode means no MCP server is reachable.
        "--strict-mcp-config".into(),
        "--no-session-persistence".into(),
        // Load no user, project, or local settings file.
        "--setting-sources".into(),
        "".into(),
        // Tools are already disabled; keep the prompting mode at its safe default
        // so nothing can be auto-approved if a future build adds an implicit tool.
        "--permission-mode".into(),
        "default".into(),
        "--max-budget-usd".into(),
        MAX_BUDGET_USD.into(),
    ]
}

/// `claude --version` prints `2.1.137 (Claude Code)`. Only the leading semantic
/// version is trusted, and it must meet the audited minimum.
pub(super) fn parse_version(bytes: &[u8]) -> Result<String, CliAdapterError> {
    let label = CliKind::Claude.label();
    let line = first_version_line(bytes, label)?;
    let raw = line
        .split_whitespace()
        .next()
        .ok_or(CliAdapterError::InvalidResponse(label))?;
    let parsed = parse_semantic_version(raw).ok_or(CliAdapterError::InvalidResponse(label))?;
    if parsed < MINIMUM_VERSION {
        return Err(CliAdapterError::UnsupportedVersion(
            label,
            raw.to_string(),
            CliKind::Claude.version_requirement(),
        ));
    }
    Ok(raw.to_string())
}

fn parse_semantic_version(raw: &str) -> Option<(u64, u64, u64)> {
    let mut parts = raw.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Claude Code wraps the model output in a result envelope. The envelope is
/// verified first, then the schema-constrained payload inside it. A payload that
/// is not the expected JSON object is still accepted as bounded text, because
/// the value is treated as untrusted inference either way.
pub(super) fn parse_answer(bytes: &[u8]) -> Result<String, CliAdapterError> {
    let label = CliKind::Claude.label();
    let envelope: ResultEnvelope =
        serde_json::from_slice(bytes).map_err(|_| CliAdapterError::InvalidResponse(label))?;
    if envelope.kind != "result" || envelope.is_error {
        return Err(CliAdapterError::InvalidResponse(label));
    }
    let payload = envelope
        .result
        .ok_or(CliAdapterError::InvalidResponse(label))?;
    let answer = serde_json::from_str::<StructuredAnswer>(&payload)
        .map(|structured| structured.answer)
        .unwrap_or(payload);
    bounded_answer(answer, label)
}
