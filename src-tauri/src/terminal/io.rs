use std::{
    io::{ErrorKind, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use portable_pty::Child;
use tauri::AppHandle;

use super::{
    events::{publish_data, publish_error, publish_exit},
    state::TerminalRegistry,
    workers::{WorkerKind, WorkerLiveness},
};

pub(super) const MAX_OUTPUT_CHUNK_BYTES: usize = 16 * 1024;
const OUTPUT_QUEUE_CAPACITY: usize = 32;
const MIN_EVENT_INTERVAL: Duration = Duration::from_millis(16);
const IO_RETRY_INTERVAL: Duration = Duration::from_millis(5);

enum PumpMessage {
    Data(Vec<u8>),
    ReaderFinished { error: Option<String> },
    Exit(Result<u32, String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReaderOutcome {
    Cancelled,
    Eof,
    Failed,
}

pub(super) fn spawn_session_io(
    app: AppHandle,
    registry: Arc<TerminalRegistry>,
    session_id: String,
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn Child + Send + Sync>,
    closed: Arc<AtomicBool>,
    workers: Arc<WorkerLiveness>,
) {
    let (sender, receiver) = mpsc::sync_channel(OUTPUT_QUEUE_CAPACITY);

    let reader_sender = sender.clone();
    let reader_closed = Arc::clone(&closed);
    let reader_workers = Arc::clone(&workers);
    let reader_registry = Arc::clone(&registry);
    let reader_session_id = session_id.clone();
    thread::spawn(move || {
        let _worker = reader_workers.enter(WorkerKind::Reader);
        let outcome = read_output(&mut reader, &reader_sender, &reader_closed);
        drop(reader);
        close_on_reader_failure(
            outcome,
            &reader_closed,
            &reader_registry,
            &reader_session_id,
        );
    });

    let wait_sender = sender.clone();
    let wait_closed = Arc::clone(&closed);
    let wait_workers = Arc::clone(&workers);
    let wait_registry = Arc::clone(&registry);
    let wait_session_id = session_id.clone();
    thread::spawn(move || {
        let _worker = wait_workers.enter(WorkerKind::Waiter);
        let result = child
            .wait()
            .map(|status| status.exit_code())
            .map_err(|error| error.to_string());
        drop(child);
        let failed = result.is_err();
        let _ = send_with_cancel(&wait_sender, PumpMessage::Exit(result), &wait_closed);
        if failed && !wait_closed.load(Ordering::Acquire) {
            wait_registry.close(&wait_session_id);
        }
    });

    drop(sender);
    thread::spawn(move || {
        let _worker = workers.enter(WorkerKind::Pump);
        pump_events(app, registry, session_id, receiver, closed);
    });
}

fn close_on_reader_failure(
    outcome: ReaderOutcome,
    closed: &AtomicBool,
    registry: &TerminalRegistry,
    session_id: &str,
) {
    if outcome == ReaderOutcome::Failed && !closed.load(Ordering::Acquire) {
        registry.close(session_id);
    }
}

fn read_output(
    reader: &mut dyn Read,
    sender: &mpsc::SyncSender<PumpMessage>,
    closed: &AtomicBool,
) -> ReaderOutcome {
    let mut buffer = vec![0_u8; MAX_OUTPUT_CHUNK_BYTES];
    let error = loop {
        if closed.load(Ordering::Acquire) {
            return ReaderOutcome::Cancelled;
        }
        match reader.read(&mut buffer) {
            Ok(0) => break None,
            Ok(read) => {
                let data = buffer[..read].to_vec();
                // Preserve byte order at the fixed queue boundary while
                // checking cancellation between bounded retries.
                if !send_with_cancel(sender, PumpMessage::Data(data), closed) {
                    return ReaderOutcome::Cancelled;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::park_timeout(IO_RETRY_INTERVAL);
            }
            Err(error) if is_terminal_eof(&error) => break None,
            Err(error) => break Some(error.to_string()),
        }
    };
    let outcome = if error.is_some() {
        ReaderOutcome::Failed
    } else {
        ReaderOutcome::Eof
    };
    let sent = send_with_cancel(sender, PumpMessage::ReaderFinished { error }, closed);
    if !sent && closed.load(Ordering::Acquire) {
        return ReaderOutcome::Cancelled;
    }
    outcome
}

fn pump_events(
    app: AppHandle,
    registry: Arc<TerminalRegistry>,
    session_id: String,
    receiver: mpsc::Receiver<PumpMessage>,
    closed: Arc<AtomicBool>,
) {
    let mut last_data_event = Instant::now()
        .checked_sub(MIN_EVENT_INTERVAL)
        .unwrap_or_else(Instant::now);
    let mut exit_code = None;

    for message in receiver {
        match message {
            PumpMessage::Data(data) => {
                if closed.load(Ordering::Acquire) {
                    continue;
                }
                let elapsed = last_data_event.elapsed();
                if elapsed < MIN_EVENT_INTERVAL {
                    thread::sleep(MIN_EVENT_INTERVAL - elapsed);
                }
                publish_data(&app, &session_id, &data);
                last_data_event = Instant::now();
            }
            PumpMessage::ReaderFinished { error } => {
                if let Some(error) = error {
                    publish_error(
                        &app,
                        &session_id,
                        &format!("Terminal output stream ended unexpectedly: {error}"),
                    );
                }
            }
            PumpMessage::Exit(Ok(code)) => exit_code = Some(code),
            PumpMessage::Exit(Err(error)) => publish_error(
                &app,
                &session_id,
                &format!("Could not observe terminal process exit: {error}"),
            ),
        }
    }

    registry.finish(&session_id);
    if let Some(code) = exit_code {
        publish_exit(&app, &session_id, code);
    }
}

fn send_with_cancel(
    sender: &mpsc::SyncSender<PumpMessage>,
    mut message: PumpMessage,
    closed: &AtomicBool,
) -> bool {
    loop {
        if closed.load(Ordering::Acquire) {
            return false;
        }
        match sender.try_send(message) {
            Ok(()) => return true,
            Err(mpsc::TrySendError::Full(returned)) => {
                message = returned;
                thread::park_timeout(IO_RETRY_INTERVAL);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => return false,
        }
    }
}

fn is_terminal_eof(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        return true;
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::EIO)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
pub(super) fn is_terminal_eof_for_test(error: &std::io::Error) -> bool {
    is_terminal_eof(error)
}

#[cfg(test)]
pub(super) fn spawn_reader_for_test(
    mut reader: Box<dyn Read + Send>,
    closed: Arc<AtomicBool>,
    workers: Arc<WorkerLiveness>,
) -> mpsc::Receiver<()> {
    let (finished_sender, finished_receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _worker = workers.enter(WorkerKind::Reader);
        let (sender, _receiver) = mpsc::sync_channel(1);
        read_output(&mut reader, &sender, &closed);
        drop(reader);
        let _ = finished_sender.send(());
    });
    finished_receiver
}

#[cfg(test)]
pub(super) fn fail_reader_for_test(
    mut reader: Box<dyn Read + Send>,
    registry: Arc<TerminalRegistry>,
    session_id: &str,
) {
    let closed = AtomicBool::new(false);
    let (sender, _receiver) = mpsc::sync_channel(1);
    let outcome = read_output(&mut reader, &sender, &closed);
    close_on_reader_failure(outcome, &closed, &registry, session_id);
}
