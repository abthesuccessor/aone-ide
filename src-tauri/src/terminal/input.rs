use std::{
    io::{ErrorKind, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use super::workers::{WorkerKind, WorkerLiveness};
use crate::error::{AoneError, AoneResult};

pub(super) const MAX_OUTSTANDING_INPUT_BYTES: usize = 256 * 1024;
pub(super) const MAX_OUTSTANDING_INPUT_MESSAGES: usize = 64;
const IO_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub(super) type TerminalWriter = Box<dyn Write + Send>;
pub(super) type WriterFailureHandler = Box<dyn FnOnce() + Send>;

struct InputBudget {
    bytes: AtomicUsize,
    messages: AtomicUsize,
    connected: AtomicBool,
}

struct QueuedInput {
    data: Vec<u8>,
    budget: Arc<InputBudget>,
}

impl Drop for QueuedInput {
    fn drop(&mut self) {
        self.budget
            .bytes
            .fetch_sub(self.data.len(), Ordering::AcqRel);
        self.budget.messages.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) struct InputQueue {
    sender: mpsc::SyncSender<QueuedInput>,
    budget: Arc<InputBudget>,
}

impl InputQueue {
    pub(super) fn start(
        writer: TerminalWriter,
        closed: Arc<AtomicBool>,
        workers: Arc<WorkerLiveness>,
        on_failure: WriterFailureHandler,
    ) -> Self {
        let budget = Arc::new(InputBudget {
            bytes: AtomicUsize::new(0),
            messages: AtomicUsize::new(0),
            connected: AtomicBool::new(true),
        });
        let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING_INPUT_MESSAGES);
        let worker_budget = Arc::clone(&budget);
        thread::spawn(move || {
            writer_loop(writer, receiver, worker_budget, closed, workers, on_failure)
        });
        Self { sender, budget }
    }

    pub(super) fn try_enqueue(&self, data: Vec<u8>) -> AoneResult<bool> {
        if !self.budget.connected.load(Ordering::Acquire) {
            return Ok(false);
        }
        if !reserve_message(&self.budget.messages) {
            return Err(backpressure_error());
        }
        if !reserve_bytes(&self.budget.bytes, data.len()) {
            self.budget.messages.fetch_sub(1, Ordering::AcqRel);
            return Err(backpressure_error());
        }

        let message = QueuedInput {
            data,
            budget: Arc::clone(&self.budget),
        };
        match self.sender.try_send(message) {
            Ok(()) => Ok(true),
            Err(mpsc::TrySendError::Full(message)) => {
                drop(message);
                Err(backpressure_error())
            }
            Err(mpsc::TrySendError::Disconnected(message)) => {
                drop(message);
                Ok(false)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn is_connected(&self) -> bool {
        self.budget.connected.load(Ordering::Acquire)
    }
}

fn writer_loop(
    mut writer: TerminalWriter,
    receiver: mpsc::Receiver<QueuedInput>,
    budget: Arc<InputBudget>,
    closed: Arc<AtomicBool>,
    workers: Arc<WorkerLiveness>,
    on_failure: WriterFailureHandler,
) {
    let _worker = workers.enter(WorkerKind::Writer);
    let _connection = ConnectionGuard(Arc::clone(&budget));
    for message in receiver {
        if closed.load(Ordering::Acquire) {
            break;
        }
        match write_message(&mut writer, &message.data, &closed) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {
                if !closed.load(Ordering::Acquire) {
                    on_failure();
                }
                break;
            }
        }
    }
    drop(writer);
}

fn write_message(
    writer: &mut TerminalWriter,
    data: &[u8],
    closed: &AtomicBool,
) -> std::io::Result<bool> {
    let mut offset = 0;
    while offset < data.len() {
        if closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        match writer.write(&data[offset..]) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(written) => offset += written,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::park_timeout(IO_RETRY_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }

    loop {
        if closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        match writer.flush() {
            Ok(()) => return Ok(true),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::park_timeout(IO_RETRY_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
}

struct ConnectionGuard(Arc<InputBudget>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.connected.store(false, Ordering::Release);
    }
}

fn reserve_message(counter: &AtomicUsize) -> bool {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < MAX_OUTSTANDING_INPUT_MESSAGES).then_some(current + 1)
        })
        .is_ok()
}

fn reserve_bytes(counter: &AtomicUsize, bytes: usize) -> bool {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current
                .checked_add(bytes)
                .filter(|next| *next <= MAX_OUTSTANDING_INPUT_BYTES)
        })
        .is_ok()
}

fn backpressure_error() -> AoneError {
    AoneError::InvalidRequest(format!(
        "terminal input backpressure: at most {MAX_OUTSTANDING_INPUT_MESSAGES} messages or {MAX_OUTSTANDING_INPUT_BYTES} bytes may be outstanding; retry after the terminal consumes input"
    ))
}
