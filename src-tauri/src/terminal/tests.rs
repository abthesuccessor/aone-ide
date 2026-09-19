use std::{
    io::{Error, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use portable_pty::{ChildKiller, MasterPty, PtySize};

use super::{
    confirmation::confirmation_message,
    environment::isolated_shell_command,
    input::{MAX_OUTSTANDING_INPUT_BYTES, MAX_OUTSTANDING_INPUT_MESSAGES},
    io::{fail_reader_for_test, is_terminal_eof_for_test},
    profiles::resolve_profile_for_test,
    request::{
        MAX_COLUMNS, MAX_INPUT_BYTES, MAX_ROWS, MIN_COLUMNS, MIN_ROWS, decode_input, pty_size,
        validate_session_id,
    },
    state::{TerminalSession, TerminalState},
};
use crate::domain::{
    TerminalEvent, TerminalEventKind, TerminalInputEncoding, TerminalOutputEncoding,
    TerminalProfile, TerminalWriteRequest,
};

#[test]
fn terminal_dimensions_are_clamped() {
    let smallest = pty_size(0, 0);
    assert_eq!(smallest.cols, MIN_COLUMNS);
    assert_eq!(smallest.rows, MIN_ROWS);

    let largest = pty_size(u16::MAX, u16::MAX);
    assert_eq!(largest.cols, MAX_COLUMNS);
    assert_eq!(largest.rows, MAX_ROWS);
}

#[test]
fn terminal_input_accepts_text_and_base64() {
    let text = TerminalWriteRequest {
        session_id: "terminal:valid".into(),
        data: "printf hello\n".into(),
        encoding: TerminalInputEncoding::Text,
    };
    assert_eq!(decode_input(&text).unwrap(), b"printf hello\n");

    let binary = TerminalWriteRequest {
        session_id: "terminal:valid".into(),
        data: STANDARD.encode([0, 1, 255]),
        encoding: TerminalInputEncoding::Base64,
    };
    assert_eq!(decode_input(&binary).unwrap(), vec![0, 1, 255]);
}

#[test]
fn terminal_input_is_bounded_before_and_after_decode() {
    let oversized_text = TerminalWriteRequest {
        session_id: "terminal:valid".into(),
        data: "x".repeat(MAX_INPUT_BYTES + 1),
        encoding: TerminalInputEncoding::Text,
    };
    assert!(decode_input(&oversized_text).is_err());

    let oversized_binary = TerminalWriteRequest {
        session_id: "terminal:valid".into(),
        data: STANDARD.encode(vec![0; MAX_INPUT_BYTES + 1]),
        encoding: TerminalInputEncoding::Base64,
    };
    assert!(decode_input(&oversized_binary).is_err());

    let malformed = TerminalWriteRequest {
        session_id: "terminal:valid".into(),
        data: "%%%".into(),
        encoding: TerminalInputEncoding::Base64,
    };
    assert!(decode_input(&malformed).is_err());
}

#[test]
fn stalled_writer_enforces_the_outstanding_byte_budget() {
    let (session, started, release, killed) = stalled_session();
    let chunk = vec![b'x'; MAX_INPUT_BYTES];
    assert!(session.write(chunk.clone()).unwrap());
    started.recv_timeout(Duration::from_secs(1)).unwrap();

    for _ in 1..(MAX_OUTSTANDING_INPUT_BYTES / MAX_INPUT_BYTES) {
        assert!(session.write(chunk.clone()).unwrap());
    }
    let error = session.write(chunk).unwrap_err();
    assert!(error.to_string().contains("terminal input backpressure"));
    assert!(session.resize(PtySize::default()).unwrap());

    session.shutdown();
    assert!(killed.load(Ordering::Acquire));
    release_writer(&release);
}

#[test]
fn message_flood_is_bounded_without_closing_the_session() {
    let (session, started, release, _killed) = stalled_session();
    assert!(session.write(vec![b'a']).unwrap());
    started.recv_timeout(Duration::from_secs(1)).unwrap();

    for _ in 1..MAX_OUTSTANDING_INPUT_MESSAGES {
        assert!(session.write(vec![b'a']).unwrap());
    }
    let error = session.write(vec![b'a']).unwrap_err();
    assert!(error.to_string().contains("64 messages"));
    assert!(session.resize(PtySize::default()).unwrap());

    session.shutdown();
    release_writer(&release);
}

#[test]
fn shutdown_never_waits_for_a_stalled_writer_worker() {
    let (session, started, release, killed) = stalled_session();
    assert!(session.write(vec![b'x']).unwrap());
    started.recv_timeout(Duration::from_secs(1)).unwrap();

    let shutdown_session = Arc::clone(&session);
    let (finished_sender, finished_receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        shutdown_session.shutdown();
        let _ = finished_sender.send(());
    });
    finished_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("shutdown waited for the blocked writer worker");

    assert!(killed.load(Ordering::Acquire));
    assert!(!session.write(vec![b'y']).unwrap());
    release_writer(&release);
}

#[test]
fn disconnected_writer_returns_not_accepted() {
    let (failed_sender, failed_receiver) = mpsc::sync_channel(1);
    let killed = Arc::new(AtomicBool::new(false));
    let session = test_session(Box::new(FailingWriter(failed_sender)), Arc::clone(&killed));
    assert!(session.write(vec![b'x']).unwrap());
    failed_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    while session.input_connected() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(!session.input_connected());
    assert!(!session.write(vec![b'y']).unwrap());
    session.shutdown();
}

#[test]
fn writer_failure_releases_the_registry_and_kills_the_session() {
    let state = TerminalState::new();
    let registry = state.registry();
    let reservation = registry.reserve().unwrap();
    let (failed_sender, failed_receiver) = mpsc::sync_channel(1);
    let killed = Arc::new(AtomicBool::new(false));
    let session = Arc::new(TerminalSession::new(
        "terminal:writer-failure".into(),
        Box::new(FailingWriter(failed_sender)),
        Box::new(TestMaster),
        Box::new(TestKiller(Arc::clone(&killed))),
        None,
        Arc::downgrade(&registry),
    ));
    reservation.activate(Arc::clone(&session)).unwrap();
    assert!(session.write(vec![b'x']).unwrap());
    failed_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    wait_until(Duration::from_secs(1), || {
        registry.session("terminal:writer-failure").is_none() && killed.load(Ordering::Acquire)
    });
    assert!(killed.load(Ordering::Acquire));
    assert!(!session.write(vec![b'y']).unwrap());
    assert!(registry.reserve().is_ok());
}

#[test]
fn reader_failure_releases_the_registry_and_kills_the_session() {
    let state = TerminalState::new();
    let registry = state.registry();
    let reservation = registry.reserve().unwrap();
    let killed = Arc::new(AtomicBool::new(false));
    let session = Arc::new(TerminalSession::new(
        "terminal:reader-failure".into(),
        Box::new(Vec::<u8>::new()),
        Box::new(TestMaster),
        Box::new(TestKiller(Arc::clone(&killed))),
        None,
        Arc::downgrade(&registry),
    ));
    reservation.activate(session).unwrap();

    fail_reader_for_test(
        Box::new(FailingReader),
        Arc::clone(&registry),
        "terminal:reader-failure",
    );
    assert!(registry.session("terminal:reader-failure").is_none());
    assert!(killed.load(Ordering::Acquire));
    assert!(registry.reserve().is_ok());
}

#[test]
fn session_ids_are_strictly_validated() {
    validate_session_id("terminal:019d-valid-123").unwrap();
    for invalid in [
        "run:019d",
        "terminal:",
        "terminal:with/slash",
        "terminal:with space",
    ] {
        assert!(validate_session_id(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn child_environment_excludes_ambient_credentials_and_loader_hooks() {
    let command = isolated_shell_command(Path::new("/bin/sh"));
    let names = command
        .iter_full_env_as_str()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();

    for forbidden in [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "AWS_SECRET_ACCESS_KEY",
        "SSH_AUTH_SOCK",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "NODE_OPTIONS",
        "PYTHONPATH",
    ] {
        assert!(!names.contains(&forbidden), "{forbidden}");
    }
    assert!(names.contains(&"SHELL"));
    assert!(names.contains(&"TERM"));
}

#[test]
fn only_one_terminal_open_may_be_reserved() {
    let state = TerminalState::new();
    let registry = state.registry();
    let reservation = registry.reserve().unwrap();
    assert!(registry.reserve().is_err());
    drop(reservation);
    assert!(registry.reserve().is_ok());
}

#[test]
fn close_all_invalidates_an_inflight_reservation() {
    let state = TerminalState::new();
    let registry = state.registry();
    let reservation = registry.reserve().unwrap();
    assert!(state.close_all());
    drop(reservation);
    assert!(registry.reserve().is_ok());
}

#[test]
fn workspace_switch_reservation_prevents_terminal_open_races() {
    let state = TerminalState::new();
    state.begin_workspace_switch().unwrap();
    assert!(state.registry().reserve().is_err());
    assert!(state.begin_workspace_switch().is_err());
    state.complete_workspace_switch();
    assert!(state.registry().reserve().is_ok());
}

#[test]
fn detected_profile_ids_are_canonical_and_deterministic() {
    let first = resolve_profile_for_test(Path::new("/bin/sh")).unwrap();
    let second = resolve_profile_for_test(Path::new("/bin/sh")).unwrap();
    assert_eq!(first.public, second.public);
    assert!(first.public.id.starts_with("terminalProfile:"));
    assert!(Path::new(&first.public.shell_path).is_absolute());
}

#[test]
fn consent_discloses_raw_io_and_omits_secret_values() {
    let profile = TerminalProfile {
        id: "terminalProfile:test".into(),
        label: "zsh".into(),
        shell_path: "/bin/zsh".into(),
    };
    let message = confirmation_message(&profile, Path::new("/tmp/project"));
    assert!(message.contains("Raw terminal input and output"));
    assert!(message.contains("not persisted"));
    assert!(message.contains("/tmp/project"));
    assert!(!message.contains("OPENAI_API_KEY"));
}

#[test]
fn terminal_event_serialization_matches_the_renderer_contract() {
    let event = TerminalEvent {
        session_id: "terminal:test".into(),
        kind: TerminalEventKind::Data,
        timestamp: "2026-08-17T00:00:00.000Z".into(),
        data: Some("AAE=".into()),
        encoding: Some(TerminalOutputEncoding::Base64),
        byte_length: Some(2),
        exit_code: None,
        detail: None,
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["sessionId"], "terminal:test");
    assert_eq!(value["kind"], "data");
    assert_eq!(value["encoding"], "base64");
    assert_eq!(value["byteLength"], 2);
    assert!(value.get("exitCode").is_none());
}

#[test]
fn unix_eio_is_treated_as_terminal_eof() {
    let unexpected = Error::from(std::io::ErrorKind::UnexpectedEof);
    assert!(is_terminal_eof_for_test(&unexpected));
    #[cfg(unix)]
    assert!(is_terminal_eof_for_test(&Error::from_raw_os_error(
        libc::EIO
    )));
}

type WriterGate = Arc<(StdMutex<bool>, Condvar)>;

struct StalledWriter {
    started: Option<mpsc::SyncSender<()>>,
    release: WriterGate,
}

impl Write for StalledWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        let (released, available) = &*self.release;
        let mut released = released.lock().unwrap();
        while !*released {
            released = available.wait(released).unwrap();
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct FailingWriter(mpsc::SyncSender<()>);

impl Write for FailingWriter {
    fn write(&mut self, _data: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.send(());
        Err(Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _data: &mut [u8]) -> std::io::Result<usize> {
        Err(Error::other("forced terminal reader failure"))
    }
}

#[derive(Debug)]
struct TestMaster;

impl MasterPty for TestMaster {
    fn resize(&self, _size: PtySize) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(PtySize::default())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        anyhow::bail!("unused test reader")
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
        anyhow::bail!("unused test writer")
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<PathBuf> {
        None
    }
}

#[derive(Debug)]
struct TestKiller(Arc<AtomicBool>);

impl ChildKiller for TestKiller {
    fn kill(&mut self) -> std::io::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self(Arc::clone(&self.0)))
    }
}

fn stalled_session() -> (
    Arc<TerminalSession>,
    mpsc::Receiver<()>,
    WriterGate,
    Arc<AtomicBool>,
) {
    let (started_sender, started_receiver) = mpsc::sync_channel(1);
    let release = Arc::new((StdMutex::new(false), Condvar::new()));
    let killed = Arc::new(AtomicBool::new(false));
    let writer = StalledWriter {
        started: Some(started_sender),
        release: Arc::clone(&release),
    };
    (
        test_session(Box::new(writer), Arc::clone(&killed)),
        started_receiver,
        release,
        killed,
    )
}

fn test_session(writer: Box<dyn Write + Send>, killed: Arc<AtomicBool>) -> Arc<TerminalSession> {
    Arc::new(TerminalSession::new(
        "terminal:test".into(),
        writer,
        Box::new(TestMaster),
        Box::new(TestKiller(killed)),
        None,
        std::sync::Weak::new(),
    ))
}

fn release_writer(gate: &WriterGate) {
    let (released, available) = &**gate;
    *released.lock().unwrap() = true;
    available.notify_all();
}

fn wait_until(timeout: Duration, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(condition(), "condition was not met before timeout");
}
