use std::{path::Path, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

use crate::error::{AoneError, AoneResult};

const CLONE_TIMEOUT: Duration = Duration::from_secs(120);
const INIT_TIMEOUT: Duration = Duration::from_secs(12);
const OUTPUT_LIMIT: usize = 16 * 1024;
const DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
/// Fixed developer-directory locations, most specific first. `/usr/bin/git` is
/// the macOS developer-tools shim; with the environment cleared it re-resolves
/// the active developer directory itself and refuses to run when that lookup
/// fails — for example when a full Xcode install has an unaccepted licence.
/// These are constants, never ambient values, so isolation is preserved.
#[cfg(target_os = "macos")]
const DEVELOPER_DIRECTORIES: &[&str] = &[
    "/Library/Developer/CommandLineTools",
    "/Applications/Xcode.app/Contents/Developer",
];

pub(super) async fn clone_repository(target: &Path, repository_url: &str) -> AoneResult<()> {
    let args = clone_arguments(repository_url);
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_git(target, &borrowed, CLONE_TIMEOUT, "clone").await
}

pub(super) async fn initialize_repository(target: &Path) -> AoneResult<()> {
    run_git(
        target,
        &["init", "--template=", "-b", "main", "."],
        INIT_TIMEOUT,
        "initialize",
    )
    .await
}

async fn run_git(
    working_directory: &Path,
    operation_args: &[&str],
    timeout: Duration,
    operation: &str,
) -> AoneResult<()> {
    let executable = trusted_git()?;
    let mut command = Command::new(executable);
    command
        .arg("--no-pager")
        .arg("--no-replace-objects")
        .arg("--no-optional-locks")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(["-c", "credential.helper="])
        .args(["-c", "protocol.file.allow=never"])
        .args(["-c", "protocol.ext.allow=never"])
        .args(["-c", "protocol.ssh.allow=never"])
        .args(["-c", "protocol.git.allow=never"])
        .args(["-c", "protocol.http.allow=never"])
        .args(["-c", "protocol.https.allow=always"])
        .args(["-c", "submodule.recurse=false"])
        .args(["-c", "filter.lfs.smudge="])
        .args(["-c", "filter.lfs.required=false"])
        .args(operation_args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1");
    #[cfg(target_os = "macos")]
    if let Some(developer_directory) = DEVELOPER_DIRECTORIES
        .iter()
        .find(|candidate| Path::new(candidate).is_dir())
    {
        command.env("DEVELOPER_DIR", developer_directory);
    }
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command
        .spawn()
        .map_err(|_| AoneError::Task(format!("Git {operation} could not be started")))?;
    let pid = child.id();
    let stdout = child.stdout.take().map(spawn_drain);
    let stderr = child.stderr.take().map(spawn_drain);
    let status = tokio::time::timeout(timeout, child.wait()).await;
    let status = match status {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => {
            stop_child(&mut child, pid).await;
            finish_drains(stdout, stderr).await;
            return Err(AoneError::Task(format!(
                "Git {operation} could not be observed"
            )));
        }
        Err(_) => {
            stop_child(&mut child, pid).await;
            finish_drains(stdout, stderr).await;
            return Err(AoneError::Task(format!(
                "Git {operation} exceeded its execution limit"
            )));
        }
    };
    terminate_process_group(pid);
    finish_drains(stdout, stderr).await;
    if !status.success() {
        return Err(AoneError::Task(format!(
            "Git {operation} failed without changing any existing project"
        )));
    }
    Ok(())
}

fn trusted_git() -> AoneResult<std::path::PathBuf> {
    let path = Path::new("/usr/bin/git");
    let canonical = path
        .canonicalize()
        .map_err(|_| AoneError::Task("/usr/bin/git is unavailable".into()))?;
    if !canonical.is_file() {
        return Err(AoneError::Task(
            "the fixed Git executable is not a regular file".into(),
        ));
    }
    Ok(canonical)
}

pub(super) fn clone_arguments(repository_url: &str) -> Vec<String> {
    [
        "clone",
        "--depth=1",
        "--single-branch",
        "--no-tags",
        "--no-recurse-submodules",
        "--",
        repository_url,
        ".",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn spawn_drain<R>(stream: R) -> tokio::task::JoinHandle<std::io::Result<usize>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(drain_bounded(stream))
}

async fn drain_bounded<R: AsyncRead + Unpin>(mut stream: R) -> std::io::Result<usize> {
    let mut retained = 0usize;
    let mut buffer = [0_u8; 4 * 1024];
    loop {
        let count = stream.read(&mut buffer).await?;
        if count == 0 {
            return Ok(retained);
        }
        retained = retained.saturating_add(count).min(OUTPUT_LIMIT);
    }
}

async fn finish_drains(
    stdout: Option<tokio::task::JoinHandle<std::io::Result<usize>>>,
    stderr: Option<tokio::task::JoinHandle<std::io::Result<usize>>>,
) {
    for mut task in [stdout, stderr].into_iter().flatten() {
        if tokio::time::timeout(DRAIN_TIMEOUT, &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
}

async fn stop_child(child: &mut tokio::process::Child, pid: Option<u32>) {
    terminate_process_group(pid);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(unix)]
fn terminate_process_group(pid: Option<u32>) {
    let Some(pid) = pid.and_then(|value| i32::try_from(value).ok()) else {
        return;
    };
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>) {}
