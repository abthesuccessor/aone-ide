use tauri::State;
use zeroize::Zeroizing;

use super::{
    cli_adapter::{CliAdapterError, CliKind, detect_cli, probe_cli_readiness},
    provider::verify_ollama_readiness,
    state::SecretState,
};
use crate::{
    domain::{
        AiCliAdapterStatus, AiConfigurationStatus, ConfigureAiCliRequest, ConfigureHostedAiRequest,
        ConfigureOllamaRequest,
    },
    error::{AoneError, AoneResult},
    state::AppState,
};

/// Installs a hosted-provider credential directly into backend-only memory.
/// The request DTO is deserialize-only and the owned credential is moved into
/// zeroizing storage before validation.
#[tauri::command]
pub fn configure_hosted_ai(
    request: ConfigureHostedAiRequest,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiConfigurationStatus> {
    let ConfigureHostedAiRequest {
        provider,
        api_key,
        model,
    } = request;
    secrets
        .configure_hosted(provider, Zeroizing::new(api_key), model)
        .map_err(AoneError::InvalidRequest)?;
    Ok(secrets.configuration_status())
}

/// Configures a local Ollama chat adapter after a bounded loopback readiness
/// check. The first inference still requires the existing native per-call
/// consent dialog.
#[tauri::command]
pub async fn configure_ollama(
    request: ConfigureOllamaRequest,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiConfigurationStatus> {
    let ConfigureOllamaRequest { endpoint, model } = request;
    configure_ollama_checked(&secrets, endpoint, model).await
}

pub(super) async fn configure_ollama_checked(
    secrets: &SecretState,
    endpoint: String,
    model: String,
) -> AoneResult<AiConfigurationStatus> {
    let configuration =
        SecretState::prepare_ollama(endpoint, model).map_err(AoneError::InvalidRequest)?;
    verify_ollama_readiness(&configuration).await?;
    // Commit only after the bounded readiness check succeeds, preserving the
    // previous provider configuration on every validation or network failure.
    secrets.install_ollama(configuration);
    Ok(secrets.configuration_status())
}

/// Reports renderer-safe adapter readiness. Every supported CLI is probed with
/// its own fixed version and authentication commands; nothing else is run.
#[tauri::command]
pub async fn list_ai_cli_adapters(
    workspaces: State<'_, AppState>,
) -> AoneResult<Vec<AiCliAdapterStatus>> {
    let workspace = workspaces.workspace_if_open().map(|current| current.root);
    Ok(list_ai_cli_adapters_checked(workspace.as_deref()).await)
}

pub(super) const SUPPORTED_CLI_KINDS: &[CliKind] = &[CliKind::Codex, CliKind::Claude];

pub(super) async fn list_ai_cli_adapters_checked(
    workspace_root: Option<&std::path::Path>,
) -> Vec<AiCliAdapterStatus> {
    let mut statuses = Vec::with_capacity(SUPPORTED_CLI_KINDS.len() + 1);
    for kind in SUPPORTED_CLI_KINDS {
        statuses.push(probe_cli_status(*kind, workspace_root).await);
    }
    statuses.push(cli_status(
        "copilot",
        "GitHub Copilot CLI",
        false,
        "not_supported",
        None,
        "The GitHub Copilot execution adapter is not implemented in this build.",
    ));
    statuses
}

async fn probe_cli_status(
    kind: CliKind,
    workspace_root: Option<&std::path::Path>,
) -> AiCliAdapterStatus {
    let (id, label) = (kind.id(), kind.label());
    match detect_cli(kind, workspace_root) {
        Ok(Some(adapter)) => match probe_cli_readiness(&adapter).await {
            Ok(readiness) => cli_status(
                id,
                label,
                true,
                "ready",
                Some(readiness.version),
                "Installed and authenticated. Aone will still ask before each inference.",
            ),
            Err(CliAdapterError::AuthenticationRequired(_, command)) => cli_status(
                id,
                label,
                true,
                "authentication_required",
                None,
                &format!("Run `{command}` in a terminal, then refresh."),
            ),
            Err(CliAdapterError::UnsupportedVersion(_, version, requirement)) => cli_status(
                id,
                label,
                true,
                "not_supported",
                Some(version),
                &format!("Install {label} {requirement} for this Aone build."),
            ),
            Err(_) => cli_status(
                id,
                label,
                true,
                "error",
                None,
                &format!(
                    "The {label} check failed safely. Retry, then inspect local authentication if it continues."
                ),
            ),
        },
        Ok(None) => cli_status(
            id,
            label,
            false,
            "not_installed",
            None,
            &format!("Install {label} in a supported native location, then refresh."),
        ),
        Err(_) => cli_status(
            id,
            label,
            true,
            "not_supported",
            None,
            &format!("A {label} file was found, but it is not a supported native executable."),
        ),
    }
}

#[tauri::command]
pub async fn configure_ai_cli(
    request: ConfigureAiCliRequest,
    workspaces: State<'_, AppState>,
    secrets: State<'_, SecretState>,
) -> AoneResult<AiConfigurationStatus> {
    let kind = CliKind::from_id(&request.adapter)
        .filter(|kind| SUPPORTED_CLI_KINDS.contains(kind))
        .ok_or_else(|| {
            AoneError::InvalidRequest(format!(
                "{} is not a supported CLI adapter in this build",
                request.adapter
            ))
        })?;
    let workspace = workspaces.workspace_if_open().map(|current| current.root);
    let adapter = detect_cli(kind, workspace.as_deref())
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| AoneError::InvalidRequest(format!("{} is not installed", kind.label())))?;
    let readiness = probe_cli_readiness(&adapter)
        .await
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    secrets.install_cli(kind, adapter, readiness.version);
    Ok(secrets.configuration_status())
}

fn cli_status(
    id: &str,
    label: &str,
    installed: bool,
    readiness: &str,
    version: Option<String>,
    detail: &str,
) -> AiCliAdapterStatus {
    AiCliAdapterStatus {
        id: id.into(),
        label: label.into(),
        installed,
        readiness: readiness.into(),
        version,
        detail: detail.into(),
    }
}
