use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use directories::UserDirs;
use serde::Deserialize;
use zeroize::Zeroizing;

use self::{
    identity::{ExecutableIdentity, canonical_directory, capture_identity},
    process::{ProcessLimits, run_process},
    temporary::TemporaryWorkspace,
};

mod claude;
mod codex;
mod identity;
mod process;
mod temporary;

const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const EXECUTION_TIMEOUT: Duration = Duration::from_secs(75);
const MAX_PROBE_OUTPUT_BYTES: usize = 12 * 1024;
const MAX_STDOUT_BYTES: usize = 128 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const MAX_PROMPT_BYTES: usize = 32 * 1024;
const MAX_ANSWER_BYTES: usize = 12 * 1024;
const JSON_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"required":["answer"],"properties":{"answer":{"type":"string","minLength":1,"maxLength":12000}}}"#;

#[derive(Debug, thiserror::Error)]
pub(super) enum CliAdapterError {
    #[error("{0} installation is not a supported native executable")]
    UnsafeExecutable(&'static str),
    #[error("{0} installation changed after it was inspected")]
    IdentityChanged(&'static str),
    #[error("{0} authentication is required; run `{1}` in a terminal")]
    AuthenticationRequired(&'static str, &'static str),
    #[error("{0} {1} is unsupported; this build requires {2}")]
    UnsupportedVersion(&'static str, String, String),
    #[error("{0} could not be started safely")]
    SpawnFailed(&'static str),
    #[error("{0} operation timed out")]
    TimedOut(&'static str),
    #[error("{0} output exceeded the local safety limit")]
    OutputLimit(&'static str),
    #[error("{0} exited unsuccessfully")]
    Unsuccessful(&'static str),
    #[error("{0} returned an invalid structured response")]
    InvalidResponse(&'static str),
    #[error("{0} temporary workspace could not be prepared")]
    TemporaryWorkspace(&'static str),
}

/// The locally installed agent CLIs Aone is willing to drive. Each one owns its
/// own discovery locations, version contract, readiness probe, fixed argument
/// list, process environment, and response shape. Nothing about a CLI is taken
/// from the renderer or from ambient configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CliKind {
    Codex,
    Claude,
}

impl CliKind {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex CLI",
            Self::Claude => "Claude Code",
        }
    }

    pub(super) fn from_id(id: &str) -> Option<Self> {
        match id {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    /// argv[0] presented to the child process.
    fn argv0(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    /// The CLI's own private configuration directory under the user's home.
    fn agent_home(self, home: &Path) -> PathBuf {
        match self {
            Self::Codex => home.join(".codex"),
            Self::Claude => home.join(".claude"),
        }
    }

    fn login_command(self) -> &'static str {
        match self {
            Self::Codex => "codex login",
            Self::Claude => "claude auth login",
        }
    }

    fn version_requirement(self) -> String {
        match self {
            Self::Codex => format!("audited version {}", codex::AUDITED_CODEX_VERSION),
            Self::Claude => {
                let (major, minor, patch) = claude::MINIMUM_VERSION;
                format!("version {major}.{minor}.{patch} or newer")
            }
        }
    }

    fn candidate_paths(self, home: &Path) -> Vec<PathBuf> {
        match self {
            Self::Codex => vec![
                home.join(".local/bin/codex"),
                home.join(".codex/packages/standalone/current/bin/codex"),
                PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex"),
                PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"),
                PathBuf::from("/opt/homebrew/bin/codex"),
                PathBuf::from("/usr/local/bin/codex"),
            ],
            Self::Claude => vec![
                home.join(".local/bin/claude"),
                home.join(".claude/local/claude"),
                PathBuf::from("/opt/homebrew/bin/claude"),
                PathBuf::from("/usr/local/bin/claude"),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CliAdapter {
    kind: CliKind,
    launch_path: PathBuf,
    identity: ExecutableIdentity,
    home: PathBuf,
    agent_home: PathBuf,
    #[cfg(test)]
    test_executable: bool,
}

impl CliAdapter {
    pub(super) fn kind(&self) -> CliKind {
        self.kind
    }

    fn home(&self) -> &Path {
        &self.home
    }

    fn agent_home(&self) -> &Path {
        &self.agent_home
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CliReadiness {
    pub(super) version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredAnswer {
    answer: String,
}

pub(super) fn detect_cli(
    kind: CliKind,
    workspace_root: Option<&Path>,
) -> Result<Option<CliAdapter>, CliAdapterError> {
    let Some(user_dirs) = UserDirs::new() else {
        return Ok(None);
    };
    let home = canonical_directory(user_dirs.home_dir())
        .unwrap_or_else(|| user_dirs.home_dir().to_path_buf());
    let configured_home = kind.agent_home(&home);
    let agent_home = canonical_directory(&configured_home).unwrap_or(configured_home);
    let workspace = workspace_root.and_then(|path| path.canonicalize().ok());
    let mut found_rejected = false;
    for launch_path in kind.candidate_paths(&home) {
        if fs::symlink_metadata(&launch_path).is_err() {
            continue;
        }
        match capture_identity(kind, &launch_path, &home, workspace.as_deref()) {
            Ok(identity) => {
                return Ok(Some(CliAdapter {
                    kind,
                    launch_path,
                    identity,
                    home,
                    agent_home,
                    #[cfg(test)]
                    test_executable: false,
                }));
            }
            Err(_) => found_rejected = true,
        }
    }
    if found_rejected {
        Err(CliAdapterError::UnsafeExecutable(kind.label()))
    } else {
        Ok(None)
    }
}

pub(super) async fn probe_cli_readiness(
    adapter: &CliAdapter,
) -> Result<CliReadiness, CliAdapterError> {
    let kind = adapter.kind;
    let temporary = TemporaryWorkspace::new(kind, false)?;
    let limits = ProcessLimits {
        timeout: PROBE_TIMEOUT,
        stdout: MAX_PROBE_OUTPUT_BYTES,
        stderr: MAX_PROBE_OUTPUT_BYTES,
    };
    let version = run_process(
        adapter,
        &[OsString::from("--version")],
        None,
        &temporary,
        limits,
    )
    .await?;
    if !version.status.success() {
        return Err(CliAdapterError::Unsuccessful(kind.label()));
    }
    let version = parse_version(kind, &version.stdout)?;

    let authentication_arguments = match kind {
        CliKind::Codex => codex::authentication_arguments(),
        CliKind::Claude => claude::authentication_arguments(),
    };
    let authentication =
        run_process(adapter, &authentication_arguments, None, &temporary, limits).await?;
    match kind {
        // Codex reports authentication solely through its exit status.
        CliKind::Codex => {
            if !authentication.status.success() {
                return Err(CliAdapterError::AuthenticationRequired(
                    kind.label(),
                    kind.login_command(),
                ));
            }
        }
        // Claude Code exits non-zero when logged out and also states it in JSON.
        // Require both to agree before treating the CLI as ready.
        CliKind::Claude => {
            let status: claude::AuthStatus = serde_json::from_slice(&authentication.stdout)
                .map_err(|_| CliAdapterError::InvalidResponse(kind.label()))?;
            if !authentication.status.success() || !status.logged_in {
                return Err(CliAdapterError::AuthenticationRequired(
                    kind.label(),
                    kind.login_command(),
                ));
            }
        }
    }
    Ok(CliReadiness { version })
}

/// Assembles the single bounded prompt a CLI adapter receives from the same
/// request body the HTTP transports use, then runs it. Keeping this beside the
/// adapter contract means the provider layer never hand-builds CLI input.
pub(super) async fn execute_cli_request(
    adapter: &CliAdapter,
    request_body: &serde_json::Value,
    workspace_root: &Path,
) -> Result<String, CliAdapterError> {
    let label = adapter.kind().label();
    let field = |name: &str| {
        request_body
            .get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or(CliAdapterError::InvalidResponse(label))
    };
    let instructions = field("instructions")?;
    let input = field("input")?;
    let prompt = Zeroizing::new(format!("{instructions}\n\n{input}"));
    execute_cli(adapter, prompt, workspace_root).await
}

pub(super) async fn execute_cli(
    adapter: &CliAdapter,
    prompt: Zeroizing<String>,
    workspace_root: &Path,
) -> Result<String, CliAdapterError> {
    execute_cli_with_limits(
        adapter,
        prompt,
        Some(workspace_root),
        ProcessLimits {
            timeout: EXECUTION_TIMEOUT,
            stdout: MAX_STDOUT_BYTES,
            stderr: MAX_STDERR_BYTES,
        },
    )
    .await
}

async fn execute_cli_with_limits(
    adapter: &CliAdapter,
    prompt: Zeroizing<String>,
    workspace_root: Option<&Path>,
    limits: ProcessLimits,
) -> Result<String, CliAdapterError> {
    let kind = adapter.kind;
    validate_prompt(kind, &prompt)?;
    if workspace_root.is_some_and(|root| executable_is_in_workspace(adapter, root)) {
        return Err(CliAdapterError::UnsafeExecutable(kind.label()));
    }
    let temporary = TemporaryWorkspace::new(kind, kind == CliKind::Codex)?;
    let args = match kind {
        CliKind::Codex => {
            let schema = temporary.path.join("answer.schema.json");
            codex::execution_arguments(&temporary.path, &schema)
        }
        CliKind::Claude => claude::execution_arguments(),
    };
    let result = run_process(adapter, &args, Some(prompt), &temporary, limits).await?;
    if !result.status.success() {
        return Err(CliAdapterError::Unsuccessful(kind.label()));
    }
    match kind {
        CliKind::Codex => codex::parse_answer(&result.stdout),
        CliKind::Claude => claude::parse_answer(&result.stdout),
    }
}

fn executable_is_in_workspace(adapter: &CliAdapter, workspace_root: &Path) -> bool {
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    adapter.launch_path.starts_with(&root) || adapter.identity.canonical_path.starts_with(root)
}

fn validate_prompt(kind: CliKind, prompt: &str) -> Result<(), CliAdapterError> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        Err(CliAdapterError::InvalidResponse(kind.label()))
    } else {
        Ok(())
    }
}

fn parse_version(kind: CliKind, bytes: &[u8]) -> Result<String, CliAdapterError> {
    match kind {
        CliKind::Codex => codex::parse_version(bytes),
        CliKind::Claude => claude::parse_version(bytes),
    }
}

fn first_version_line(bytes: &[u8], label: &'static str) -> Result<String, CliAdapterError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
        .map(str::trim)
        .filter(|line| line.len() <= 120 && !line.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or(CliAdapterError::InvalidResponse(label))
}

fn bounded_answer(answer: String, label: &'static str) -> Result<String, CliAdapterError> {
    let answer = answer.trim();
    if answer.is_empty() || answer.len() > MAX_ANSWER_BYTES {
        return Err(CliAdapterError::InvalidResponse(label));
    }
    Ok(answer.to_string())
}

#[cfg(test)]
#[path = "cli_adapter_tests.rs"]
mod tests;
