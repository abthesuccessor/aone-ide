use std::future::Future;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::{destination::literal_private_warning, request::PreparedWebSocket};
use crate::error::{AoneError, AoneResult};

pub(super) async fn confirm_connection(
    app: &AppHandle,
    request: &PreparedWebSocket,
) -> AoneResult<()> {
    let warns_private_literal = literal_private_warning(&request.url);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(confirmation_message(request))
        .title("Confirm WebSocket connection")
        .kind(if warns_private_literal {
            MessageDialogKind::Warning
        } else {
            MessageDialogKind::Info
        })
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Connect".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });

    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AoneError::InvalidRequest(
            "WebSocket connection was cancelled".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "WebSocket confirmation dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn confirmation_message(request: &PreparedWebSocket) -> String {
    let headers = if request.header_names.is_empty() {
        "None".to_owned()
    } else {
        request.header_names.join(", ")
    };
    let warning = if literal_private_warning(&request.url) {
        "\n\nWARNING: This destination reaches localhost or a private network. It may access services that are not exposed publicly."
    } else {
        ""
    };
    format!(
        "Aone IDE is ready to open a persistent connection.\n\nDestination: {}\nHeader names: {}\nRequested subprotocols: {}{}\n\nHeader values, URL query values, and message data are intentionally hidden. DNS lookup and all network activity begin only after approval.",
        request.consent_destination(),
        headers,
        request.protocols.len(),
        warning,
    )
}

pub(super) async fn resolve_after_approval<T, Approval, Resolver, Resolution>(
    approval: Approval,
    resolver: Resolver,
) -> AoneResult<T>
where
    Approval: Future<Output = AoneResult<()>>,
    Resolver: FnOnce() -> Resolution,
    Resolution: Future<Output = AoneResult<T>>,
{
    approval.await?;
    resolver().await
}
