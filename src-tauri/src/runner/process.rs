use crate::error::{AoneError, AoneResult};

#[derive(Debug, Clone, Copy)]
pub(super) enum StopSignal {
    Terminate,
    Kill,
}

#[cfg(unix)]
pub(super) fn signal_process_tree(pid: u32, signal: StopSignal) -> AoneResult<()> {
    let signal = match signal {
        StopSignal::Terminate => libc::SIGTERM,
        StopSignal::Kill => libc::SIGKILL,
    };
    let pid = i32::try_from(pid)
        .map_err(|_| AoneError::Task("process id is outside the supported range".into()))?;
    // The child is launched as its own process group, so a negative PID targets
    // the complete descendant group on macOS and other Unix platforms.
    let result = unsafe { libc::kill(-pid, signal) };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        Err(AoneError::Task(format!(
            "failed to signal process group: {}",
            error
        )))
    }
}

#[cfg(not(unix))]
pub(super) fn signal_process_tree(_pid: u32, _signal: StopSignal) -> AoneResult<()> {
    Err(AoneError::Task(
        "process-tree stopping is not implemented on this platform".into(),
    ))
}
