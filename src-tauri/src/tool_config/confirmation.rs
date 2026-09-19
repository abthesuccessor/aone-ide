use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::catalog::ToolSpec;
use crate::error::{AoneError, AoneResult};

pub(super) async fn confirm_configuration_inspection(app: &AppHandle) -> AoneResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(inspection_confirmation_message())
        .title("Inspect AI tool configurations")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Inspect AI tools".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "AI tool inspection was cancelled".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "AI tool inspection dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn inspection_confirmation_message() -> &'static str {
    "Aone will inspect metadata for a fixed list of AI-tool configuration files, project instructions, agent skills, known application locations, and executable names directly in at most 64 PATH directories.\n\nConfiguration contents, source contents, environment values, and credentials stay hidden. Aone will not run an executable, invoke a shell, or recursively scan this Mac."
}

pub(super) async fn confirm_configuration_open(
    app: &AppHandle,
    spec: &ToolSpec,
    will_create: bool,
    approved_path: &Path,
) -> AoneResult<()> {
    let message = configuration_confirmation_message(spec, will_create, approved_path);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .title("Open developer tool configuration")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open configuration".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "tool configuration open was cancelled".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "tool configuration confirmation dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn configuration_confirmation_message(
    spec: &ToolSpec,
    will_create: bool,
    approved_path: &Path,
) -> String {
    let creation = if will_create {
        "Aone will first create a minimal empty configuration with owner-only file permissions."
    } else {
        "The existing file will not be changed by Aone."
    };
    format!(
        "Open the {} configuration in the macOS default text editor?\n\nOwner: {}\nConfigured location: {}\nCanonical target: {approved_path:?}\n\n{}\n\nThis file may contain API keys or commands. Aone never reads its contents into the WebView.",
        spec.label, spec.company, spec.path_hint, creation,
    )
}
