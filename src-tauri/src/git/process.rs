use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

use crate::error::{AoneError, AoneResult};

use super::{
    environment::{isolate_configuration, scrub_git_environment},
    inspection::InspectionSnapshot,
};

const GIT_TIMEOUT: Duration = Duration::from_secs(12);
const STDERR_LIMIT: usize = 16 * 1024;
#[derive(Debug)]
pub(super) struct GitOutput {
    pub(super) success: bool,
    pub(super) code: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stdout_truncated: bool,
}

pub(super) async fn run_git(
    workspace_root: &Path,
    args: &[String],
    stdout_limit: usize,
) -> AoneResult<GitOutput> {
    let executable = resolve_git_executable(workspace_root)?;
    execute_git(&executable, workspace_root, args, stdout_limit, None, false).await
}

pub(super) async fn run_git_isolated(
    workspace_root: &Path,
    args: &[String],
    stdout_limit: usize,
) -> AoneResult<GitOutput> {
    let executable = resolve_git_executable(workspace_root)?;
    execute_git(&executable, workspace_root, args, stdout_limit, None, true).await
}

pub(super) async fn run_git_inspection(
    workspace_root: &Path,
    args: &[String],
    stdout_limit: usize,
) -> AoneResult<GitOutput> {
    let executable = resolve_git_executable(workspace_root)?;
    let snapshot = InspectionSnapshot::capture(workspace_root, &executable).await?;
    execute_git(
        &executable,
        workspace_root,
        args,
        stdout_limit,
        Some(&snapshot),
        true,
    )
    .await
}

async fn execute_git(
    executable: &Path,
    workspace_root: &Path,
    args: &[String],
    stdout_limit: usize,
    inspection: Option<&InspectionSnapshot>,
    isolated_configuration: bool,
) -> AoneResult<GitOutput> {
    let mut command = Command::new(executable);
    command
        .arg("--no-pager")
        .arg("--no-replace-objects")
        .arg("--no-lazy-fetch")
        .arg("--no-optional-locks")
        .arg("--literal-pathspecs")
        .args(["-c", "core.fsmonitor=false"])
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(["-c", "commit.gpgSign=false"])
        .args(["-c", "diff.external="])
        .args(["-c", "core.attributesFile=/dev/null"])
        .args(["-c", "core.excludesFile=/dev/null"])
        .args(["-c", "credential.helper="]);
    if let Some(snapshot) = inspection {
        snapshot.add_global_arguments(&mut command, workspace_root);
    } else {
        command.arg("-C").arg(workspace_root);
    }
    command
        .args(args)
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("TERM", "dumb");
    scrub_git_environment(&mut command);
    if let Some(snapshot) = inspection {
        snapshot.isolate_environment(&mut command);
    } else if isolated_configuration {
        isolate_configuration(&mut command);
    }

    let mut child = command
        .spawn()
        .map_err(|error| AoneError::Task(format!("failed to start local Git: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AoneError::Task("local Git stdout was not captured".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AoneError::Task("local Git stderr was not captured".into()))?;

    let completed = timeout(GIT_TIMEOUT, async {
        let (stdout, _stderr, status) = tokio::join!(
            read_bounded(stdout, stdout_limit),
            read_bounded(stderr, STDERR_LIMIT),
            child.wait(),
        );
        Ok::<_, std::io::Error>((stdout?, status?))
    })
    .await;

    let ((stdout, stdout_truncated), status) = match completed {
        Ok(result) => result.map_err(AoneError::Io)?,
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(AoneError::Task(
                "local Git exceeded the 12 second execution limit".into(),
            ));
        }
    };
    Ok(GitOutput {
        success: status.success(),
        code: status.code(),
        stdout,
        stdout_truncated,
    })
}

async fn read_bounded<R>(mut reader: R, limit: usize) -> std::io::Result<(Vec<u8>, bool)>
where
    R: AsyncRead + Unpin,
{
    let mut kept = Vec::with_capacity(limit.min(16 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let available = limit.saturating_sub(kept.len());
        let retained = count.min(available);
        kept.extend_from_slice(&buffer[..retained]);
        truncated |= retained < count;
    }
    Ok((kept, truncated))
}

fn resolve_git_executable(workspace_root: &Path) -> AoneResult<PathBuf> {
    let candidate = Path::new("/usr/bin/git");
    let canonical = candidate
        .canonicalize()
        .map_err(|_| AoneError::Task("/usr/bin/git is not installed".into()))?;
    if !canonical.is_file() || canonical.starts_with(workspace_root) {
        return Err(AoneError::Task(
            "the fixed local Git executable is not a trusted file".into(),
        ));
    }
    Ok(canonical)
}

pub(super) fn git_failure(operation: &str, output: &GitOutput) -> AoneError {
    let code = output
        .code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "signal".into());
    AoneError::Task(format!("local Git {operation} failed (exit {code})"))
}
