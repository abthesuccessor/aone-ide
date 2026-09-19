use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    domain::TerminalProfile,
    error::{AoneError, AoneResult},
};

pub(super) async fn confirm_terminal_open(
    app: &AppHandle,
    profile: &TerminalProfile,
    cwd: &Path,
) -> AoneResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(confirmation_message(profile, cwd))
        .title("Confirm interactive terminal")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open terminal".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });

    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "terminal open was cancelled by the user".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "terminal confirmation dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn confirmation_message(profile: &TerminalProfile, cwd: &Path) -> String {
    format!(
        "Aone IDE is ready to open an interactive terminal.\n\nProfile: {}\nShell: {}\nWorking directory: {}\n\nWARNING: Raw terminal input and output are visible to Aone IDE while this session is open. They are not persisted, indexed, sent to AI, or added to the graph. Loaded project environment values are not injected.",
        profile.label,
        profile.shell_path,
        cwd.display(),
    )
}
