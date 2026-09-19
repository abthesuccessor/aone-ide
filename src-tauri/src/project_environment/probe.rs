use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    io,
    process::Stdio,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

use super::{
    catalog::{TOOL_CATALOG, tool_spec},
    discovery::{DiscoveredTools, ExecutableCandidate},
    identity::revalidate_executable_identity,
};

const MAX_PROBES: usize = 16;
const MAX_APPROVAL_DETAIL_BYTES: usize = 3_500;
const MAX_CAPTURE_BYTES: usize = 12 * 1024;
const MAX_VERSION_BYTES: usize = 240;
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const PROBE_CONCURRENCY: usize = 4;
const DRAIN_JOIN_TIMEOUT: Duration = Duration::from_millis(350);

#[derive(Debug, Clone)]
pub(super) struct PlannedProbe {
    pub(super) candidate: ExecutableCandidate,
    pub(super) approval_line: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ProbePlan {
    pub(super) probes: Vec<PlannedProbe>,
    pub(super) omitted_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ProbeResult {
    pub(super) version: Option<String>,
    pub(super) error: Option<String>,
}

pub(super) fn build_probe_plan(
    discovered: &DiscoveredTools,
    requested_ids: &BTreeSet<&'static str>,
) -> ProbePlan {
    let mut plan = ProbePlan::default();
    let mut detail_bytes: usize = 0;
    for spec in TOOL_CATALOG {
        if !requested_ids.contains(spec.id) {
            continue;
        }
        let Some(candidate) = discovered.get(spec.id).and_then(|items| items.first()) else {
            continue;
        };
        let argv = std::iter::once(candidate.launch_name)
            .chain(spec.version_args.iter().copied())
            .collect::<Vec<_>>();
        let line = format!(
            "{}\n  Canonical executable: {:?}\n  Fixed argv: {:?}",
            spec.label,
            candidate.canonical_path(),
            argv,
        );
        if plan.probes.len() == MAX_PROBES
            || detail_bytes.saturating_add(line.len()) > MAX_APPROVAL_DETAIL_BYTES
        {
            plan.omitted_count += 1;
            continue;
        }
        detail_bytes += line.len();
        plan.probes.push(PlannedProbe {
            candidate: candidate.clone(),
            approval_line: line,
        });
    }
    plan
}

pub(super) async fn run_probe_plan(plan: &ProbePlan) -> BTreeMap<&'static str, ProbeResult> {
    let mut results = BTreeMap::new();
    for chunk in plan.probes.chunks(PROBE_CONCURRENCY) {
        let tasks = chunk
            .iter()
            .cloned()
            .map(|planned| {
                tokio::spawn(async move {
                    let tool_id = planned.candidate.tool_id;
                    (tool_id, probe_one(planned.candidate).await)
                })
            })
            .collect::<Vec<_>>();
        for task in tasks {
            match task.await {
                Ok((tool_id, result)) => {
                    results.insert(tool_id, result);
                }
                Err(_) => {
                    // A task panic cannot be associated with renderer input;
                    // the remaining bounded probes still complete normally.
                }
            }
        }
    }
    results
}

async fn probe_one(candidate: ExecutableCandidate) -> ProbeResult {
    if revalidate_executable_identity(&candidate.launch_path, &candidate.identity).is_err() {
        return failed("Executable identity changed after approval.");
    }
    let Some(spec) = tool_spec(candidate.tool_id) else {
        return failed("Version check is unavailable for this tool.");
    };
    let mut command = Command::new(candidate.canonical_path());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.as_std_mut().arg0(candidate.launch_name);
    }
    command
        .args(spec.version_args)
        .current_dir(probe_working_directory())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .env_clear()
        .env("PATH", probe_path(&candidate))
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .env("FLUTTER_SUPPRESS_ANALYTICS", "true");
    #[cfg(unix)]
    command.process_group(0);

    let Ok(mut child) = command.spawn() else {
        return failed("Version command could not be started.");
    };
    let pid = child.id();
    let stdout_task = child
        .stdout
        .take()
        .map(|stream| tokio::spawn(drain_bounded(stream)));
    let stderr_task = child
        .stderr
        .take()
        .map(|stream| tokio::spawn(drain_bounded(stream)));

    let status = match tokio::time::timeout(PROBE_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => {
            return finish_failed(
                child,
                pid,
                stdout_task,
                stderr_task,
                "Version command could not be observed.",
            )
            .await;
        }
        Err(_) => {
            return finish_failed(
                child,
                pid,
                stdout_task,
                stderr_task,
                "Version command timed out.",
            )
            .await;
        }
    };
    // A version command has no reason to leave background descendants. Kill
    // the approved process group after its leader exits so inherited pipe
    // handles cannot keep output drains alive indefinitely.
    terminate_process_group(pid);
    let stdout = join_output(stdout_task).await;
    let stderr = join_output(stderr_task).await;
    if !status.success() {
        return failed("Version command exited unsuccessfully.");
    }
    let version = normalized_first_line(&stdout).or_else(|| normalized_first_line(&stderr));
    match version {
        Some(version) => ProbeResult {
            version: Some(version),
            error: None,
        },
        None => failed("Version command returned no printable version."),
    }
}

async fn finish_failed(
    mut child: tokio::process::Child,
    pid: Option<u32>,
    stdout_task: Option<tokio::task::JoinHandle<io::Result<Vec<u8>>>>,
    stderr_task: Option<tokio::task::JoinHandle<io::Result<Vec<u8>>>>,
    message: &'static str,
) -> ProbeResult {
    terminate_process_group(pid);
    let _ = child.start_kill();
    let _ = child.wait().await;
    let _ = join_output(stdout_task).await;
    let _ = join_output(stderr_task).await;
    failed(message)
}

async fn drain_bounded<R: AsyncRead + Unpin>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut kept = Vec::with_capacity(MAX_CAPTURE_BYTES);
    let mut buffer = [0_u8; 2_048];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(kept);
        }
        let remaining = MAX_CAPTURE_BYTES.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}

async fn join_output(task: Option<tokio::task::JoinHandle<io::Result<Vec<u8>>>>) -> Vec<u8> {
    let Some(mut task) = task else {
        return Vec::new();
    };
    match tokio::time::timeout(DRAIN_JOIN_TIMEOUT, &mut task).await {
        Ok(result) => result.ok().and_then(Result::ok).unwrap_or_default(),
        Err(_) => {
            task.abort();
            let _ = task.await;
            Vec::new()
        }
    }
}

fn normalized_first_line(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    for raw_line in text.lines() {
        let mut line = String::new();
        for character in raw_line.trim().chars() {
            if character.is_control() {
                continue;
            }
            let normalized = if character.is_whitespace() {
                ' '
            } else {
                character
            };
            if line.len() + normalized.len_utf8() > MAX_VERSION_BYTES {
                break;
            }
            line.push(normalized);
        }
        if !line.is_empty() {
            return Some(line);
        }
    }
    None
}

fn failed(message: &'static str) -> ProbeResult {
    ProbeResult {
        version: None,
        error: Some(message.into()),
    }
}

fn probe_path(candidate: &ExecutableCandidate) -> OsString {
    let mut directories = Vec::new();
    if let Some(parent) = candidate.launch_path.parent() {
        directories.push(parent);
    }
    directories.extend([
        std::path::Path::new("/usr/bin"),
        std::path::Path::new("/bin"),
    ]);
    std::env::join_paths(directories).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

fn probe_working_directory() -> &'static std::path::Path {
    #[cfg(unix)]
    {
        std::path::Path::new("/")
    }
    #[cfg(not(unix))]
    {
        std::path::Path::new("C:\\")
    }
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

#[cfg(test)]
pub(super) fn normalize_version_for_test(bytes: &[u8]) -> Option<String> {
    normalized_first_line(bytes)
}

#[cfg(test)]
pub(super) async fn join_output_for_test(
    task: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
) -> Vec<u8> {
    join_output(Some(task)).await
}
