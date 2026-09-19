use portable_pty::MasterPty;

use crate::error::{AoneError, AoneResult};

/// Portable PTY duplicates share the same open-file status flags on Unix.
/// Configure the master before cloning reader/writer handles so every worker
/// gets cancellable, non-blocking I/O.
#[cfg(unix)]
pub(super) fn configure_nonblocking(master: &dyn MasterPty) -> AoneResult<()> {
    let fd = master
        .as_raw_fd()
        .ok_or_else(|| AoneError::Task("terminal master has no Unix file descriptor".into()))?;
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(AoneError::Io(std::io::Error::last_os_error()));
    }
    if flags & libc::O_NONBLOCK == 0 {
        let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if result == -1 {
            return Err(AoneError::Io(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn configure_nonblocking(_master: &dyn MasterPty) -> AoneResult<()> {
    Err(AoneError::InvalidRequest(
        "integrated terminal requires interruptible PTY I/O and is currently supported only on Unix"
            .into(),
    ))
}
