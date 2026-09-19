use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::error::{AoneError, AoneResult};

pub(super) async fn confirm_git_mutation(
    app: &AppHandle,
    title: &str,
    message: String,
    action_label: &str,
) -> AoneResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            action_label.into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "local Git action was cancelled".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "local Git confirmation dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn path_confirmation_message(
    operation: &str,
    workspace_root: &std::path::Path,
    paths: &[String],
    may_run_filters: bool,
) -> String {
    let visible = paths
        .iter()
        .take(20)
        .map(|path| format!("  {path:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let remaining = paths.len().saturating_sub(20);
    let remainder = if remaining == 0 {
        String::new()
    } else {
        format!("\n  ... and {remaining} more")
    };
    let filter_warning = if may_run_filters {
        "\n\nRepository or user Git clean filters may run local code while staging. Aone disables Git hooks and never contacts remotes."
    } else {
        "\n\nAone disables Git hooks and never contacts remotes."
    };
    format!(
        "Aone will {operation} local Git paths.\n\nWorkspace:\n{workspace_root:?}\n\nPaths:\n{visible}{remainder}{filter_warning}"
    )
}

pub(super) fn commit_confirmation_message(
    workspace_root: &std::path::Path,
    message: &str,
    staged_paths: &[String],
) -> String {
    let preview = message.chars().take(160).collect::<String>();
    let suffix = if message.chars().count() > 160 {
        "..."
    } else {
        ""
    };
    let visible_paths = staged_paths
        .iter()
        .take(20)
        .map(|path| format!("  {path:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    let remaining = staged_paths.len().saturating_sub(20);
    let remainder = if remaining == 0 {
        String::new()
    } else {
        format!("\n  ... and {remaining} more")
    };
    format!(
        "Aone will create a local commit from every currently staged change.\n\nWorkspace:\n{workspace_root:?}\n\nStaged paths:\n{visible_paths}{remainder}\n\nCommit message preview:\n{preview:?}{suffix}\n\nGit hooks and commit signing are disabled. Aone never pushes or contacts a remote."
    )
}

pub(super) fn init_confirmation_message(workspace_root: &std::path::Path) -> String {
    format!(
        "Aone will create a local .git repository in this workspace.\n\nWorkspace:\n{workspace_root:?}\n\nNo remote is added and no files are committed."
    )
}
