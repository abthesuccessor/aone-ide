use std::{future::Future, time::Duration};

use reqwest::Url;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tokio::sync::oneshot;

use super::{
    destination::{literal_loopback, literal_private_warning},
    request::PreparedRequest,
};
use crate::error::{AoneError, AoneResult};

const OUTBOUND_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutboundAuthorization {
    ExplicitLoopbackSend,
    NativeConfirmation,
}

pub(super) fn outbound_authorization(url: &Url) -> OutboundAuthorization {
    if literal_loopback(url) {
        OutboundAuthorization::ExplicitLoopbackSend
    } else {
        OutboundAuthorization::NativeConfirmation
    }
}

pub(super) async fn confirm_outbound_request(
    app: &AppHandle,
    request: &PreparedRequest,
) -> AoneResult<()> {
    let warns_private_literal = literal_private_warning(&request.url);
    let (sender, receiver) = oneshot::channel();
    app.dialog()
        .message(outbound_confirmation_message(request))
        .title("Confirm outbound API request")
        .kind(if warns_private_literal {
            MessageDialogKind::Warning
        } else {
            MessageDialogKind::Info
        })
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Send request".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });

    await_outbound_confirmation(receiver, OUTBOUND_CONFIRMATION_TIMEOUT).await
}

pub(super) async fn await_outbound_confirmation(
    receiver: oneshot::Receiver<bool>,
    confirmation_timeout: Duration,
) -> AoneResult<()> {
    match tokio::time::timeout(confirmation_timeout, receiver).await {
        Ok(Ok(true)) => Ok(()),
        Ok(Ok(false)) => Err(AoneError::InvalidRequest(
            "outbound API request was cancelled".into(),
        )),
        Ok(Err(_)) => Err(AoneError::Task(
            "outbound API confirmation dialog closed unexpectedly".into(),
        )),
        Err(_) => Err(AoneError::Task(
            "outbound API confirmation timed out before network activity".into(),
        )),
    }
}

pub(super) fn outbound_confirmation_message(request: &PreparedRequest) -> String {
    let warning = if literal_private_warning(&request.url) {
        "\n\nWARNING: This destination reaches localhost or a private network. It may access services that are not exposed publicly."
    } else {
        ""
    };
    format!(
        "Aone IDE is ready to send this request.\n\nMethod: {}\nOrigin: {}\nURL: {}{}\n\nHeader and body values are intentionally hidden. DNS lookup and all network activity begin only after approval.",
        request.method,
        request.url.origin().ascii_serialization(),
        confirmation_url(&request.url),
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

fn confirmation_url(url: &Url) -> String {
    let mut safe = url.clone();
    if url.query().is_some() {
        let names = url
            .query_pairs()
            .map(|(name, _)| name.into_owned())
            .collect::<Vec<_>>();
        safe.set_query(None);
        safe.query_pairs_mut()
            .extend_pairs(names.iter().map(|name| (name.as_str(), "[value hidden]")));
    }
    safe.to_string()
}
