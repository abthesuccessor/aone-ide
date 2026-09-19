use std::{collections::BTreeMap, sync::Arc, time::Duration};

use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{Mutex, watch},
    time::Instant,
};
use zeroize::Zeroizing;

use super::{
    RuntimeState,
    debugger::{DebugLine, DebugSessionContext, ingest_debug_line},
    publish_event, redact_text,
    session::RunStreamSession,
    trace::{TRACE_PREFIX, TraceLine, TraceSession, ingest_trace_line},
};
use crate::domain::EvidenceKind;

pub(super) const MAX_STREAM_LINE_BYTES: usize = 16 * 1024;
pub(super) const MAX_OUTPUT_EVENTS_PER_SECOND: u32 = 50;
const TRUNCATED_TRACE_REASON: &str = "structured trace line exceeds the 16 KiB limit";
const TRUNCATED_DEBUG_REASON: &str = "structured debug line exceeds the 16 KiB limit";
const DEBUG_RUNNING_UI_SAMPLE_RATE: u64 = 10;

pub(super) type SharedOutputEmissionGate = Arc<Mutex<OutputEmissionGate>>;

pub(super) struct OutputEmissionGate {
    next_emission: Instant,
    interval: Duration,
}

impl OutputEmissionGate {
    fn new() -> Self {
        Self::with_start(Instant::now())
    }

    fn with_start(start: Instant) -> Self {
        Self {
            next_emission: start,
            interval: Duration::from_secs(1) / MAX_OUTPUT_EVENTS_PER_SECOND,
        }
    }

    fn reserve_delay(&mut self, now: Instant) -> Option<Duration> {
        let scheduled = self.next_emission.max(now);
        self.next_emission = scheduled + self.interval;
        let delay = scheduled.saturating_duration_since(now);
        (!delay.is_zero()).then_some(delay)
    }

    async fn wait_for_turn(&mut self) {
        if let Some(delay) = self.reserve_delay(Instant::now()) {
            tokio::time::sleep(delay).await;
        }
    }
}

pub(super) fn shared_output_emission_gate() -> SharedOutputEmissionGate {
    Arc::new(Mutex::new(OutputEmissionGate::new()))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_stream_reader<R>(
    app: AppHandle,
    run_session: RunStreamSession,
    kind: &'static str,
    label: &'static str,
    stream: R,
    secret_values: Arc<Vec<Zeroizing<String>>>,
    emission_gate: SharedOutputEmissionGate,
    trace_session: Option<Arc<TraceSession>>,
    debug_session: Option<Arc<DebugSessionContext>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::with_capacity(MAX_STREAM_LINE_BYTES);
        let mut cancellation = run_session.cancellation();
        let mut debug_protocol_failed = false;
        let mut debug_running_seen = 0_u64;
        loop {
            let read = tokio::select! {
                _ = cancelled(&mut cancellation) => break,
                read = read_bounded_line(&mut reader, &mut bytes, MAX_STREAM_LINE_BYTES) => read,
            };
            match read {
                Ok(None) => break,
                Ok(Some(truncated)) => {
                    let runtime = app.state::<RuntimeState>();
                    if !runtime.run_session_is_current(&run_session) {
                        break;
                    }
                    let raw_text = String::from_utf8_lossy(&bytes)
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    if let Some((debug_session, bounded)) =
                        debug_session.as_deref().zip(bounded_protocol_input(
                            &raw_text,
                            truncated,
                            super::debugger::DEBUG_PREFIX,
                            TRUNCATED_DEBUG_REASON,
                        ))
                    {
                        if debug_protocol_failed {
                            continue;
                        }
                        let debug_line = bounded.map_or_else(DebugLine::Rejected, |line| {
                            ingest_debug_line(
                                &app,
                                &runtime,
                                debug_session,
                                &run_session,
                                line,
                                secret_values.as_slice(),
                            )
                        });
                        match debug_line {
                            DebugLine::Accepted(mut event) => {
                                let running = event
                                    .metadata
                                    .get("safePointState")
                                    .and_then(serde_json::Value::as_str)
                                    == Some("running");
                                if running {
                                    debug_running_seen = debug_running_seen.saturating_add(1);
                                }
                                let publish = !running
                                    || debug_running_seen == 1
                                    || debug_running_seen
                                        .is_multiple_of(DEBUG_RUNNING_UI_SAMPLE_RATE);
                                if publish {
                                    event.metadata.insert(
                                        "uiRunningSampleRate".into(),
                                        json!(DEBUG_RUNNING_UI_SAMPLE_RATE),
                                    );
                                    if runtime
                                        .with_current_run_session(&run_session, || {
                                            publish_event(&app, &runtime, event)
                                        })
                                        .is_none()
                                    {
                                        break;
                                    }
                                }
                                continue;
                            }
                            DebugLine::Rejected(reason) => {
                                debug_protocol_failed = true;
                                runtime.fail_debug_control(
                                    run_session.run_id(),
                                    format!("cooperative debug protocol failed: {reason}"),
                                );
                                let mut metadata = BTreeMap::new();
                                metadata.insert("reason".into(), json!(reason));
                                metadata.insert("debugProtocol".into(), json!("AONE_DEBUG_V1"));
                                let event = runtime.next_event(
                                    Some(run_session.run_id().to_owned()),
                                    "debug.rejected",
                                    "Cooperative debug safe point rejected",
                                    EvidenceKind::Observed,
                                    Some(run_session.run_id().to_owned()),
                                    metadata,
                                );
                                if runtime
                                    .with_current_run_session(&run_session, || {
                                        publish_event(&app, &runtime, event)
                                    })
                                    .is_none()
                                {
                                    break;
                                }
                                continue;
                            }
                            DebugLine::NotDebug => unreachable!(
                                "a classified debug marker must be parsed as debug input"
                            ),
                        }
                    }
                    if let Some((trace_session, bounded)) =
                        trace_session.as_deref().zip(bounded_protocol_input(
                            &raw_text,
                            truncated,
                            TRACE_PREFIX,
                            TRUNCATED_TRACE_REASON,
                        ))
                    {
                        let trace_line = bounded.map_or_else(TraceLine::Rejected, |line| {
                            ingest_trace_line(
                                &app,
                                &runtime,
                                trace_session,
                                &run_session,
                                line,
                                secret_values.as_slice(),
                            )
                        });
                        match trace_line {
                            TraceLine::Accepted(event) => {
                                if !wait_for_turn_or_cancel(&emission_gate, &mut cancellation).await
                                {
                                    break;
                                }
                                if runtime
                                    .with_current_run_session(&run_session, || {
                                        publish_event(&app, &runtime, event)
                                    })
                                    .is_none()
                                {
                                    break;
                                }
                                continue;
                            }
                            TraceLine::Rejected(reason) => {
                                if !wait_for_turn_or_cancel(&emission_gate, &mut cancellation).await
                                {
                                    break;
                                }
                                let mut metadata = BTreeMap::new();
                                metadata.insert("reason".into(), json!(reason));
                                metadata.insert("traceProtocol".into(), json!("AONE_TRACE_V1"));
                                let event = runtime.next_event(
                                    Some(run_session.run_id().to_owned()),
                                    "trace.rejected",
                                    "Structured trace line rejected",
                                    EvidenceKind::Observed,
                                    Some(run_session.run_id().to_owned()),
                                    metadata,
                                );
                                if runtime
                                    .with_current_run_session(&run_session, || {
                                        publish_event(&app, &runtime, event)
                                    })
                                    .is_none()
                                {
                                    break;
                                }
                                continue;
                            }
                            TraceLine::NotTrace => unreachable!(
                                "a classified trace marker must be parsed as trace input"
                            ),
                        }
                    }
                    let text = redact_text(&raw_text, secret_values.as_slice());
                    // Stdout and stderr share this FIFO gate. Each reader holds
                    // at most one bounded logical line while waiting, allowing
                    // the OS pipes to apply backpressure without altering text
                    // bytes or ANSI control sequences accepted by the reader.
                    if !wait_for_turn_or_cancel(&emission_gate, &mut cancellation).await {
                        break;
                    }
                    let mut metadata = BTreeMap::new();
                    metadata.insert("text".into(), json!(text));
                    metadata.insert("truncated".into(), json!(truncated));
                    let event = runtime.next_event(
                        Some(run_session.run_id().to_owned()),
                        kind,
                        label,
                        EvidenceKind::Observed,
                        Some(run_session.run_id().to_owned()),
                        metadata,
                    );
                    if runtime
                        .with_current_run_session(&run_session, || {
                            publish_event(&app, &runtime, event)
                        })
                        .is_none()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let runtime = app.state::<RuntimeState>();
                    let mut metadata = BTreeMap::new();
                    metadata.insert("stream".into(), json!(label));
                    metadata.insert("error".into(), json!(error.to_string()));
                    let event = runtime.next_event(
                        Some(run_session.run_id().to_owned()),
                        "process.stream_failed",
                        "Process output stream failed",
                        EvidenceKind::Observed,
                        Some(run_session.run_id().to_owned()),
                        metadata,
                    );
                    let _ = runtime.with_current_run_session(&run_session, || {
                        publish_event(&app, &runtime, event)
                    });
                    break;
                }
            }
        }
    });
}

fn bounded_protocol_input<'a>(
    line: &'a str,
    truncated: bool,
    prefix: &str,
    truncated_reason: &'static str,
) -> Option<Result<&'a str, &'static str>> {
    line.starts_with(prefix).then_some(if truncated {
        Err(truncated_reason)
    } else {
        Ok(line)
    })
}

async fn wait_for_turn_or_cancel(
    gate: &SharedOutputEmissionGate,
    cancellation: &mut watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        _ = cancelled(cancellation) => false,
        _ = async { gate.lock().await.wait_for_turn().await } => true,
    }
}

async fn cancelled(cancellation: &mut watch::Receiver<bool>) {
    if *cancellation.borrow() {
        return;
    }
    let _ = cancellation.changed().await;
}

/// Reads one logical line without allowing the output buffer to grow beyond
/// `limit`. Bytes beyond the limit are discarded until a newline or EOF so the
/// following call always starts at a new logical line.
pub(super) async fn read_bounded_line<R>(
    reader: &mut R,
    output: &mut Vec<u8>,
    limit: usize,
) -> std::io::Result<Option<bool>>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    output.clear();
    let mut saw_bytes = false;
    let mut truncated = false;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(saw_bytes.then_some(truncated));
        }
        saw_bytes = true;

        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        let chunk = &available[..consumed];
        let remaining = limit.saturating_sub(output.len());
        let copied = remaining.min(chunk.len());
        output.extend_from_slice(&chunk[..copied]);
        truncated |= copied < chunk.len();
        let ended_line = chunk.last() == Some(&b'\n');
        reader.consume(consumed);

        if ended_line {
            return Ok(Some(truncated));
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn shared_gate_reserves_one_combined_fifo_slot_every_twenty_milliseconds() {
        let start = Instant::now();
        let mut gate = OutputEmissionGate::with_start(start);
        let interval = Duration::from_millis(20);

        assert_eq!(MAX_OUTPUT_EVENTS_PER_SECOND, 50);
        assert_eq!(gate.reserve_delay(start), None);
        assert_eq!(gate.reserve_delay(start), Some(interval));
        assert_eq!(gate.reserve_delay(start), Some(interval * 2));
    }

    #[test]
    fn delayed_consumers_do_not_receive_a_catch_up_burst() {
        let start = Instant::now();
        let mut gate = OutputEmissionGate::with_start(start);
        assert_eq!(gate.reserve_delay(start), None);

        let delayed = start + Duration::from_secs(2);
        assert_eq!(gate.reserve_delay(delayed), None);
        assert_eq!(gate.reserve_delay(delayed), Some(Duration::from_millis(20)));
    }

    #[tokio::test]
    async fn bounded_reader_preserves_ansi_and_utf8_payloads() {
        let input = "\u{1b}[32m準備 ready\u{1b}[0m\n";
        let mut reader = BufReader::new(input.as_bytes());
        let mut output = Vec::new();

        assert_eq!(
            read_bounded_line(&mut reader, &mut output, MAX_STREAM_LINE_BYTES)
                .await
                .unwrap(),
            Some(false)
        );
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\u{1b}[32m準備 ready\u{1b}[0m\n"
        );
    }

    #[tokio::test]
    async fn oversized_prefix_valid_trace_is_rejected_before_json_ingestion() {
        let valid = r#"AONE_TRACE_V1 {"nonce":"n","traceId":"t","spanId":"s","kind":"event","phase":"event","label":"work","source":{"relativePath":"src/main.rs","line":1}}"#;
        let mut input = valid.as_bytes().to_vec();
        input.resize(MAX_STREAM_LINE_BYTES + 32, b' ');
        input.push(b'\n');
        let mut reader = BufReader::new(input.as_slice());
        let mut output = Vec::new();
        let truncated = read_bounded_line(&mut reader, &mut output, MAX_STREAM_LINE_BYTES)
            .await
            .unwrap()
            .unwrap();
        let retained = String::from_utf8(output).unwrap();

        assert!(truncated);
        assert_eq!(
            bounded_protocol_input(&retained, truncated, TRACE_PREFIX, TRUNCATED_TRACE_REASON,),
            Some(Err(TRUNCATED_TRACE_REASON))
        );
    }

    #[test]
    fn oversized_ordinary_output_is_not_misclassified_as_a_protocol_failure() {
        let ordinary = "x".repeat(MAX_STREAM_LINE_BYTES);
        assert_eq!(
            bounded_protocol_input(
                &ordinary,
                true,
                super::super::debugger::DEBUG_PREFIX,
                TRUNCATED_DEBUG_REASON,
            ),
            None
        );
        assert_eq!(
            bounded_protocol_input(&ordinary, true, TRACE_PREFIX, TRUNCATED_TRACE_REASON),
            None
        );
    }

    #[test]
    fn oversized_debug_marker_is_a_debug_protocol_failure() {
        let marker = format!("{}{{", super::super::debugger::DEBUG_PREFIX);
        assert_eq!(
            bounded_protocol_input(
                &marker,
                true,
                super::super::debugger::DEBUG_PREFIX,
                TRUNCATED_DEBUG_REASON,
            ),
            Some(Err(TRUNCATED_DEBUG_REASON))
        );
    }
}
