use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use portable_pty::{PtySize, native_pty_system};

use super::{
    input::InputQueue, io::spawn_reader_for_test, pty_io::configure_nonblocking,
    request::MAX_INPUT_BYTES, workers::WorkerLiveness,
};

#[test]
fn retained_real_pty_slave_cannot_keep_io_workers_or_cloned_fds_alive() {
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    configure_nonblocking(pair.master.as_ref()).unwrap();
    let master_fd = pair.master.as_raw_fd().unwrap();
    let flags = unsafe { libc::fcntl(master_fd, libc::F_GETFL) };
    assert_ne!(flags & libc::O_NONBLOCK, 0);
    let reader = pair.master.try_clone_reader().unwrap();
    let writer = pair.master.take_writer().unwrap();
    // Keep the real slave open and unread. This models a detached descendant
    // retaining the slave after the original shell/process group is gone.
    let _retained_slave = pair.slave;

    let reader_dropped = Arc::new(AtomicBool::new(false));
    let writer_dropped = Arc::new(AtomicBool::new(false));
    let closed = Arc::new(AtomicBool::new(false));
    let workers = WorkerLiveness::new();
    let input = InputQueue::start(
        Box::new(DropObservedWriter {
            inner: writer,
            dropped: Arc::clone(&writer_dropped),
        }),
        Arc::clone(&closed),
        Arc::clone(&workers),
        Box::new(|| {}),
    );
    let reader_finished = spawn_reader_for_test(
        Box::new(DropObservedReader {
            inner: reader,
            dropped: Arc::clone(&reader_dropped),
        }),
        Arc::clone(&closed),
        Arc::clone(&workers),
    );

    wait_until(Duration::from_secs(1), || workers.active_mask() & 3 == 3);
    assert!(input.try_enqueue(vec![b'x'; MAX_INPUT_BYTES]).unwrap());
    closed.store(true, Ordering::Release);
    drop(input);

    reader_finished
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    wait_until(Duration::from_secs(1), || workers.active_mask() == 0);
    assert!(reader_dropped.load(Ordering::Acquire));
    assert!(writer_dropped.load(Ordering::Acquire));
}

struct DropObservedWriter {
    inner: Box<dyn Write + Send>,
    dropped: Arc<AtomicBool>,
}

impl Write for DropObservedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.inner.write(data)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Drop for DropObservedWriter {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

struct DropObservedReader {
    inner: Box<dyn Read + Send>,
    dropped: Arc<AtomicBool>,
}

impl Read for DropObservedReader {
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(data)
    }
}

impl Drop for DropObservedReader {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn wait_until(timeout: Duration, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(condition(), "condition was not met before timeout");
}
