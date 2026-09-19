use std::{collections::HashMap, path::Path};

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroize;

use crate::{
    ai::SecretState,
    domain::EnvLoadResult,
    environment::load_attested_env_values,
    error::{AoneError, AoneResult},
    runner::RuntimeState,
};

/// Opens a backend-owned native picker for the optional AI provider. Only the
/// selected OpenAI or Anthropic key and model are retained, and neither can
/// enter the child-process environment through this command.
#[tauri::command]
pub async fn pick_and_load_env_file(
    app: AppHandle,
    secrets: State<'_, SecretState>,
) -> AoneResult<Option<EnvLoadResult>> {
    let selected = app
        .dialog()
        .file()
        .set_title("Select a private environment file for the AI provider")
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    let values = load_attested_env_values(&path)?;
    install_provider_env(values, &secrets).map(Some)
}

/// Opens a separate native picker for variables explicitly made available to
/// run profiles. Provider credentials are removed even if they share the file.
#[tauri::command]
pub async fn pick_and_load_run_env_file(
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<Option<EnvLoadResult>> {
    let expected_root = runtime.workspace_root()?;
    let selected = app
        .dialog()
        .file()
        .set_title("Select an environment file for local run profiles")
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| AoneError::InvalidRequest(error.to_string()))?;
    let values = load_attested_env_values(&path)?;
    install_run_env(values, &runtime, &expected_root).map(Some)
}

fn is_provider_env(name: &str) -> bool {
    matches!(
        name,
        "AONE_AI_PROVIDER"
            | "OPENAI_API_KEY"
            | "OPENAI_MODEL"
            | "ANTHROPIC_API_KEY"
            | "ANTHROPIC_MODEL"
    )
}

fn env_result(values: &HashMap<String, String>) -> EnvLoadResult {
    let mut names = values.keys().cloned().collect::<Vec<_>>();
    names.sort();
    EnvLoadResult {
        loaded_count: names.len(),
        names,
    }
}

pub(super) fn install_provider_env(
    mut values: HashMap<String, String>,
    secrets: &SecretState,
) -> AoneResult<EnvLoadResult> {
    values.retain(|name, value| {
        let retain = is_provider_env(name);
        if !retain {
            value.zeroize();
        }
        retain
    });
    if let Err(error) = validate_provider_env_selection(&values) {
        zeroize_values(&mut values);
        return Err(error);
    }
    let result = env_result(&values);
    secrets
        .replace_env(values)
        .map_err(AoneError::InvalidRequest)?;
    Ok(result)
}

fn zeroize_values(values: &mut HashMap<String, String>) {
    for value in values.values_mut() {
        value.zeroize();
    }
}

fn validate_provider_env_selection(values: &HashMap<String, String>) -> AoneResult<()> {
    let selected = values.get("AONE_AI_PROVIDER").map(|value| value.trim());
    let has_openai = values
        .get("OPENAI_API_KEY")
        .is_some_and(|value| !value.trim().is_empty());
    let has_anthropic = values
        .get("ANTHROPIC_API_KEY")
        .is_some_and(|value| !value.trim().is_empty());

    match selected {
        Some("openai") if !has_openai => Err(AoneError::InvalidRequest(
            "AONE_AI_PROVIDER selects openai, but OPENAI_API_KEY is missing".into(),
        )),
        Some("anthropic") if !has_anthropic => Err(AoneError::InvalidRequest(
            "AONE_AI_PROVIDER selects anthropic, but ANTHROPIC_API_KEY is missing".into(),
        )),
        Some("") | None if !has_openai && !has_anthropic => Err(AoneError::InvalidRequest(
            "no supported provider key found; add OPENAI_API_KEY or ANTHROPIC_API_KEY".into(),
        )),
        _ => Ok(()),
    }
}

pub(super) fn install_run_env(
    mut values: HashMap<String, String>,
    runtime: &RuntimeState,
    expected_root: &Path,
) -> AoneResult<EnvLoadResult> {
    values.retain(|name, value| {
        let retain = !is_provider_env(name);
        if !retain {
            value.zeroize();
        }
        retain
    });
    let result = env_result(&values);
    runtime.replace_env_for_workspace(expected_root, values)?;
    Ok(result)
}
