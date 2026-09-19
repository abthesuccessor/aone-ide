use std::{
    ffi::OsString,
    process::{ExitStatus, Stdio},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::Zeroizing;

use super::{
    CliAdapter, CliAdapterError, CliKind, identity::revalidate_identity,
    temporary::TemporaryWorkspace,
};

pub(super) struct ProcessOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(super) struct ProcessLimits {
    pub(super) timeout: Duration,
    pub(super) stdout: usize,
    pub(super) stderr: usize,
}

pub(super) async fn run_process(
    adapter: &CliAdapter,
    args: &[OsString],
    input: Option<Zeroizing<String>>,
    temporary: &TemporaryWorkspace,
    limits: ProcessLimits,
) -> Result<ProcessOutput, CliAdapterError> {
    let label = adapter.kind().label();
    revalidate_identity(adapter)?;
    let mut command = Command::new(&adapter.identity.canonical_path);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.as_std_mut().arg0(adapter.kind().argv0());
    }
    command
        .args(args)
        .current_dir(&temporary.path)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .env_clear()
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .env("LANG", "C")
        .env("LC_ALL", "C");
    apply_agent_environment(&mut command, adapter);
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command
        .spawn()
        .map_err(|_| CliAdapterError::SpawnFailed(label))?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or(CliAdapterError::SpawnFailed(label))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(CliAdapterError::SpawnFailed(label))?;
    let stdin = child.stdin.take();
    let operation = async {
        let (status, stdout, _, ()) = tokio::try_join!(
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| CliAdapterError::Unsuccessful(label))
            },
            drain_limited(stdout, limits.stdout, label),
            drain_limited(stderr, limits.stderr, label),
            write_input(stdin, input, label),
        )?;
        Ok::<_, CliAdapterError>(ProcessOutput { status, stdout })
    };
    match tokio::time::timeout(limits.timeout, operation).await {
        Ok(Ok(output)) => {
            terminate_process_group(pid);
            Ok(output)
        }
        Ok(Err(error)) => {
            terminate_child(&mut child, pid).await;
            Err(error)
        }
        Err(_) => {
            terminate_child(&mut child, pid).await;
            Err(CliAdapterError::TimedOut(label))
        }
    }
}

/// Per-CLI environment, applied on top of a cleared environment. Only the
/// variables a CLI genuinely needs to find its own credentials and configuration
/// are reintroduced, and each one is set to a value Aone computed — never to an
/// inherited value.
fn apply_agent_environment(command: &mut Command, adapter: &CliAdapter) {
    match adapter.kind() {
        CliKind::Codex => {
            command.env("CODEX_HOME", adapter.agent_home());
        }
        CliKind::Claude => {
            // Claude Code resolves its credentials through the OS keychain and
            // its configuration directory, both of which are located relative to
            // HOME. `--setting-sources ""` still prevents any settings file from
            // being loaded, so this grants credential access and nothing else.
            command
                .env("HOME", adapter.home())
                .env("CLAUDE_CONFIG_DIR", adapter.agent_home())
                // A fixed minimal PATH: every tool is disabled, so the CLI has
                // no legitimate reason to resolve executables from the user's
                // shell environment.
                .env("PATH", "/usr/bin:/bin");
        }
    }
}

async fn write_input(
    stdin: Option<tokio::process::ChildStdin>,
    input: Option<Zeroizing<String>>,
    label: &'static str,
) -> Result<(), CliAdapterError> {
    let (Some(mut stdin), Some(input)) = (stdin, input) else {
        return Ok(());
    };
    let bytes = Zeroizing::new(input.as_bytes().to_vec());
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| CliAdapterError::Unsuccessful(label))?;
    stdin
        .shutdown()
        .await
        .map_err(|_| CliAdapterError::Unsuccessful(label))
}

async fn drain_limited<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
    label: &'static str,
) -> Result<Vec<u8>, CliAdapterError> {
    let mut output = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = reader
            .read(&mut buffer)
            .await
            .map_err(|_| CliAdapterError::Unsuccessful(label))?;
        if count == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(count) > limit {
            return Err(CliAdapterError::OutputLimit(label));
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

async fn terminate_child(child: &mut tokio::process::Child, pid: Option<u32>) {
    terminate_process_group(pid);
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
}

#[cfg(unix)]
fn terminate_process_group(pid: Option<u32>) {
    if let Some(pid) = pid.and_then(|value| i32::try_from(value).ok()) {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>) {}
