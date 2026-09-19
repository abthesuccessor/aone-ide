//! Codex CLI specifics: the pinned audited version, the sandbox feature
//! denylist, the exact argument surface, and the structured response shape.

use std::{ffi::OsString, path::Path};

use super::{CliAdapterError, CliKind, StructuredAnswer, bounded_answer, first_version_line};

pub(super) const AUDITED_CODEX_VERSION: &str = "0.144.6";

pub(super) const DISABLED_FEATURES: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "shell_snapshot",
    "plugins",
    "apps",
    "memories",
    "multi_agent",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "computer_use",
    "image_generation",
    "goals",
    "in_app_browser",
    "code_mode_host",
    "tool_suggest",
    "workspace_dependencies",
    "hooks",
    "auth_elicitation",
    "skill_mcp_dependency_install",
    "plugin_sharing",
    "remote_plugin",
];

pub(super) fn authentication_arguments() -> Vec<OsString> {
    vec![OsString::from("login"), OsString::from("status")]
}

pub(super) fn execution_arguments(working_directory: &Path, schema: &Path) -> Vec<OsString> {
    let mut args = vec![
        "--ask-for-approval".into(),
        "never".into(),
        "exec".into(),
        "--ephemeral".into(),
        "--ignore-user-config".into(),
        "--ignore-rules".into(),
        "--skip-git-repo-check".into(),
        "--strict-config".into(),
        "--color".into(),
        "never".into(),
        "-C".into(),
        working_directory.as_os_str().to_owned(),
        "-c".into(),
        "default_permissions=\"aone_bounded\"".into(),
        "-c".into(),
        "permissions.aone_bounded.filesystem={\":minimal\"=\"read\",\":project\"=\"read\"}".into(),
        "-c".into(),
        "permissions.aone_bounded.network.enabled=false".into(),
    ];
    for feature in DISABLED_FEATURES {
        args.push("--disable".into());
        args.push((*feature).into());
    }
    args.extend([
        "--output-schema".into(),
        schema.as_os_str().to_owned(),
        "-".into(),
    ]);
    args
}

pub(super) fn parse_version(bytes: &[u8]) -> Result<String, CliAdapterError> {
    let label = CliKind::Codex.label();
    let line = first_version_line(bytes, label)?;
    let raw = line
        .strip_prefix("codex-cli ")
        .ok_or(CliAdapterError::InvalidResponse(label))?;
    if raw != AUDITED_CODEX_VERSION {
        return Err(CliAdapterError::UnsupportedVersion(
            label,
            raw.to_string(),
            CliKind::Codex.version_requirement(),
        ));
    }
    Ok(raw.to_string())
}

pub(super) fn parse_answer(bytes: &[u8]) -> Result<String, CliAdapterError> {
    let label = CliKind::Codex.label();
    let parsed: StructuredAnswer =
        serde_json::from_slice(bytes).map_err(|_| CliAdapterError::InvalidResponse(label))?;
    bounded_answer(parsed.answer, label)
}
