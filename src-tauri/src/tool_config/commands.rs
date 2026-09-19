use std::{process::Stdio, time::Duration};

use tauri::{AppHandle, State};
use tokio::{process::Command, sync::Mutex, time::timeout};

use super::{
    catalog::{
        InspectionContext, describe_inspected, describe_static, find_spec, resolve_path, specs,
    },
    confirmation::{confirm_configuration_inspection, confirm_configuration_open},
    files::{ConfigTargetAttestation, attest_target, create_config_atomically, revalidate_target},
};
use crate::{
    domain::{ToolConfiguration, ToolConfigurationOpenRequest, ToolConfigurationOpenResult},
    error::{AoneError, AoneResult},
    state::AppState,
};

static TOOL_CONFIG_OPEN_LOCK: Mutex<()> = Mutex::const_new(());
static TOOL_CONFIG_INSPECTION_LOCK: Mutex<()> = Mutex::const_new(());

#[tauri::command]
pub fn list_tool_configurations() -> Vec<ToolConfiguration> {
    specs().iter().map(describe_static).collect()
}

#[tauri::command]
pub async fn inspect_tool_configurations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<Vec<ToolConfiguration>> {
    let _inspection_guard = reserve_inspection()?;
    confirm_configuration_inspection(&app).await?;
    let context = InspectionContext::capture();
    Ok(specs()
        .iter()
        .map(|spec| describe_inspected(spec, &state, &context))
        .collect())
}

pub(super) fn reserve_inspection() -> AoneResult<tokio::sync::MutexGuard<'static, ()>> {
    TOOL_CONFIG_INSPECTION_LOCK.try_lock().map_err(|_| {
        AoneError::InvalidRequest("an AI tool inspection is already awaiting approval".into())
    })
}

#[tauri::command]
pub async fn open_tool_configuration(
    request: ToolConfigurationOpenRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<ToolConfigurationOpenResult> {
    let _open_guard = TOOL_CONFIG_OPEN_LOCK.lock().await;
    let spec = find_spec(&request.tool_id)?;
    let (trusted_root, target) = resolve_path(spec, &state)?;
    let approved_target = attest_target(&trusted_root, &target)?;
    let existed = approved_target.exists();
    if !existed && !request.create_if_missing {
        return Ok(ToolConfigurationOpenResult {
            opened: false,
            created: false,
            path_hint: spec.path_hint.into(),
        });
    }

    confirm_configuration_open(&app, spec, !existed, approved_target.approved_path()).await?;
    let (current_root, current_target) = resolve_path(spec, &state)?;
    if current_root != trusted_root || current_target != target {
        return Err(AoneError::InvalidRequest(
            "workspace changed while the configuration was awaiting confirmation".into(),
        ));
    }
    let (created, final_target) =
        prepare_final_target(&trusted_root, &target, spec.template, approved_target)?;
    open_in_macos_text_editor(&trusted_root, &target, &final_target).await?;
    Ok(ToolConfigurationOpenResult {
        opened: true,
        created,
        path_hint: spec.path_hint.into(),
    })
}

fn prepare_final_target(
    trusted_root: &std::path::Path,
    target: &std::path::Path,
    template: &str,
    approved: ConfigTargetAttestation,
) -> AoneResult<(bool, ConfigTargetAttestation)> {
    if approved.exists() {
        return Ok((false, approved));
    }
    revalidate_target(trusted_root, target, &approved)?;
    if !create_config_atomically(trusted_root, target, template)? {
        return Err(AoneError::InvalidRequest(
            "tool configuration appeared after confirmation; review it before opening".into(),
        ));
    }
    let created = attest_target(trusted_root, target)?;
    if !created.exists() {
        return Err(AoneError::Task(
            "created tool configuration could not be attested".into(),
        ));
    }
    Ok((true, created))
}

async fn open_in_macos_text_editor(
    trusted_root: &std::path::Path,
    target: &std::path::Path,
    expected: &ConfigTargetAttestation,
) -> AoneResult<()> {
    let executable = std::path::Path::new("/usr/bin/open")
        .canonicalize()
        .map_err(|_| AoneError::Task("macOS /usr/bin/open is unavailable".into()))?;
    // Keep this identity and containment check immediately adjacent to spawn.
    // No renderer-controlled operation occurs between attestation and launch.
    let current = revalidate_target(trusted_root, target, expected)?;
    let canonical_target = current.existing_canonical_path()?;
    let mut child = Command::new(executable)
        .arg("-t")
        .arg(canonical_target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| AoneError::Task(format!("failed to open text editor: {error}")))?;
    let status = match timeout(Duration::from_secs(10), child.wait()).await {
        Ok(result) => result.map_err(AoneError::Io)?,
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(AoneError::Task(
                "macOS text-editor launch exceeded 10 seconds".into(),
            ));
        }
    };
    if !status.success() {
        return Err(AoneError::Task(format!(
            "macOS text-editor launch failed (exit {})",
            status
                .code()
                .map_or_else(|| "signal".into(), |code| code.to_string())
        )));
    }
    Ok(())
}
