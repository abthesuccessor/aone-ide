use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::error::{AoneError, AoneResult};

pub(super) async fn confirm_clone(
    app: &AppHandle,
    repository_url: &str,
    destination: &Path,
) -> AoneResult<bool> {
    show(
        app,
        "Clone public GitHub project",
        format!(
            "Aone will contact GitHub and create a new project in Documents.\n\nRepository:\n{repository_url}\n\nDestination:\n{destination:?}\n\nThe clone is shallow and single-branch, and checks out every ordinary tracked file in the current snapshot. Submodules and Git LFS downloads are not initialized. Git prompts, credentials, hooks, global configuration, and local or extended protocols are disabled. Nothing is pushed or configured on GitHub.",
        ),
        "Clone project",
    )
    .await
}

pub(super) async fn confirm_create(
    app: &AppHandle,
    destination: &Path,
    initialize_git: bool,
) -> AoneResult<bool> {
    let git_detail = if initialize_git {
        "A local Git repository with a main branch will also be initialized. No remote is added and nothing is committed or pushed."
    } else {
        "Git will not be initialized."
    };
    show(
        app,
        "Create project in Documents",
        format!(
            "Aone will create a new folder and README.md.\n\nDestination:\n{destination:?}\n\n{git_detail}\n\nAn existing file or folder will never be overwritten.",
        ),
        "Create project",
    )
    .await
}

pub(super) async fn confirm_git_inspection(
    app: &AppHandle,
    workspace_name: &str,
    workspace_root: &Path,
) -> AoneResult<bool> {
    show(
        app,
        "Inspect Git setup",
        format!(
            "Aone can inspect bounded Git onboarding metadata for the currently open workspace.\n\nWorkspace: {workspace_name:?}\nLocation: {workspace_root:?}\n\nIf you continue, Aone may read .git/config, .git/HEAD, global Git identity files, and up to 16 regular public .pub key files under ~/.ssh. Remote credentials, query strings, comments, and local paths are redacted.\n\nAone never opens private key files. It will not run Git, SSH, GitHub CLI, contact a network, fork, create a branch, push, or modify configuration.",
        ),
        "Inspect Git setup",
    )
    .await
}

async fn show(
    app: &AppHandle,
    title: &str,
    message: String,
    approve_label: &str,
) -> AoneResult<bool> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            approve_label.into(),
            "Cancel".into(),
        ))
        .show(move |approved| {
            let _ = sender.send(approved);
        });
    receiver
        .await
        .map_err(|_| AoneError::Task("onboarding confirmation closed unexpectedly".into()))
}
