use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::{
    config_hints::{MAX_HINT_FILE_BYTES, MAX_HINT_FILES, MAX_HINT_TOTAL_BYTES},
    probe::ProbePlan,
};
use crate::error::{AoneError, AoneResult};

pub(super) async fn confirm_environment_inspection(
    app: &AppHandle,
    workspace_name: &str,
    workspace_root: &Path,
) -> AoneResult<bool> {
    show_confirmation(
        app,
        "Inspect project environment",
        discovery_confirmation_message(workspace_name, workspace_root),
        "Inspect environment",
        "Cancel",
    )
    .await
}

pub(super) async fn confirm_version_probes(app: &AppHandle, plan: &ProbePlan) -> AoneResult<bool> {
    if plan.probes.is_empty() {
        return Ok(false);
    }
    show_confirmation(
        app,
        "Confirm local version checks",
        probe_confirmation_message(plan),
        "Run version checks",
        "Skip version checks",
    )
    .await
}

pub(super) fn discovery_confirmation_message(
    workspace_name: &str,
    workspace_root: &Path,
) -> String {
    format!(
        "Aone can inspect the local development environment for this open project.\n\n\
         Workspace: {:?}\n\
         Location: {:?}\n\n\
         If you continue, Aone will:\n\
         - inspect the already-open project's language summary and bounded run manifests;\n\
         - read a fixed allowlist of already-indexed build/config files (at most {} files, {} KiB each, {} KiB total) to derive names-only environment and PostgreSQL/Redis dependency hints;\n\
         - inspect the inherited absolute PATH entries plus fixed standard and common user tool directories;\n\
         - resolve a fixed allowlist of development executables to canonical paths.\n\n\
         No executable will run during this step. Aone will not open project env files, known credential paths, shell startup files, or AI/CLI tool configuration contents. Build/config values are not returned or retained, and every environment/dependency result is an inference rather than a required-service or readiness claim. Aone will not install, download, configure, or modify anything.",
        workspace_name,
        workspace_root,
        MAX_HINT_FILES,
        MAX_HINT_FILE_BYTES / 1024,
        MAX_HINT_TOTAL_BYTES / 1024,
    )
}

pub(super) fn probe_confirmation_message(plan: &ProbePlan) -> String {
    let mut message = String::from(
        "Aone found relevant local executables. Review every command below before allowing fixed, read-only version checks.\n\n",
    );
    for planned in &plan.probes {
        message.push_str(&planned.approval_line);
        message.push_str("\n\n");
    }
    if plan.omitted_count > 0 {
        message.push_str(&format!(
            "{} additional relevant tool(s) were omitted by the safety limit and will not run.\n\n",
            plan.omitted_count,
        ));
    }
    message.push_str(
        "Only these listed commands will run. Each uses a clean environment, bounded output, a timeout, and process-group cleanup. No install, download, project command, login shell, or project env value is allowed. Choose Skip to keep the detected paths unverified.",
    );
    message
}

async fn show_confirmation(
    app: &AppHandle,
    title: &str,
    message: String,
    approve_label: &str,
    decline_label: &str,
) -> AoneResult<bool> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            approve_label.into(),
            decline_label.into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    receiver.await.map_err(|_| {
        AoneError::Task("project environment confirmation dialog closed unexpectedly".into())
    })
}
