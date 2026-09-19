use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::{
    io::AsyncWriteExt,
    process::ChildStdin,
    sync::{mpsc, watch},
};

use super::protocol::DebugSessionContext;
use crate::{
    domain::DebugControlAction,
    runner::{RuntimeState, session::RunStreamSession},
};

pub(in crate::runner) const DEBUG_CONTROL_ACK_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);
const DEBUG_CONTROL_PREFIX: &str = "AONE_DEBUG_CONTROL_V1 ";

#[derive(Debug, Clone)]
pub(in crate::runner) struct QueuedDebugControl {
    pub(in crate::runner) action: DebugControlAction,
    pub(in crate::runner) control_epoch: u64,
    pub(in crate::runner) expected_sequence: u64,
}

pub(in crate::runner) type DebugControlSender = mpsc::Sender<QueuedDebugControl>;
pub(in crate::runner) type DebugControlReceiver = mpsc::Receiver<QueuedDebugControl>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DebugControlWire<'a> {
    nonce: &'a str,
    debug_session_id: &'a str,
    action: DebugControlAction,
    control_epoch: u64,
    expected_sequence: u64,
}

pub(in crate::runner) fn debug_control_channel() -> (DebugControlSender, DebugControlReceiver) {
    mpsc::channel(16)
}

pub(in crate::runner) fn spawn_debug_control_writer(
    app: AppHandle,
    run_session: RunStreamSession,
    mut stdin: ChildStdin,
    mut receiver: DebugControlReceiver,
    context: Arc<DebugSessionContext>,
) {
    tauri::async_runtime::spawn(async move {
        let mut cancellation = run_session.cancellation();
        loop {
            let control = tokio::select! {
                _ = cancelled(&mut cancellation) => break,
                control = receiver.recv() => control,
            };
            let Some(control) = control else { break };
            let runtime = app.state::<RuntimeState>();
            if !runtime.run_session_is_current(&run_session) {
                break;
            }
            let encoded = encode_control_line(&context, &control);
            if let Err(error) = stdin.write_all(&encoded).await {
                runtime.fail_debug_control(
                    run_session.run_id(),
                    format!("failed to deliver cooperative debug control: {error}"),
                );
                break;
            }
            if let Err(error) = stdin.flush().await {
                runtime.fail_debug_control(
                    run_session.run_id(),
                    format!("failed to flush cooperative debug control: {error}"),
                );
                break;
            }
        }
    });
}

pub(in crate::runner) fn encode_control_line(
    context: &DebugSessionContext,
    control: &QueuedDebugControl,
) -> Vec<u8> {
    let payload = DebugControlWire {
        nonce: context.nonce.as_str(),
        debug_session_id: &context.debug_session_id,
        action: control.action,
        control_epoch: control.control_epoch,
        expected_sequence: control.expected_sequence,
    };
    let mut encoded = DEBUG_CONTROL_PREFIX.as_bytes().to_vec();
    encoded.extend(serde_json::to_vec(&payload).expect("fixed debug control DTO must serialize"));
    encoded.push(b'\n');
    encoded
}

async fn cancelled(cancellation: &mut watch::Receiver<bool>) {
    if *cancellation.borrow() {
        return;
    }
    let _ = cancellation.changed().await;
}
