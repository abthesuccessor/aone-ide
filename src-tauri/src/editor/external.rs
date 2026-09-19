use std::{
    env,
    fs::Metadata,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime},
};

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use super::{
    formatting::{FORMATTERS, FormatterPlan},
    validation::{MAX_EDITABLE_FILE_BYTES, validate_source_content},
};
use crate::{
    domain::FormatterCapability,
    error::{AoneError, AoneResult},
};

const FORMAT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_FORMATTER_STDERR_BYTES: usize = 64 * 1024;
const SAFE_PARENT_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

#[derive(Debug)]
pub(super) struct ResolvedFormatter {
    launch_path: PathBuf,
    target: PathBuf,
    identity: FileIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExternalFormatOutcome {
    Formatted,
    Failed,
}

#[derive(Debug)]
struct FileIdentity {
    length: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

pub(super) fn formatter_capabilities() -> Vec<FormatterCapability> {
    let mut capabilities = vec![FormatterCapability {
        language: "All text".into(),
        formatter: super::formatting::BUILTIN_FORMATTER.into(),
        available: true,
        external: false,
    }];
    for definition in FORMATTERS {
        for language in definition.languages {
            capabilities.push(FormatterCapability {
                language: (*language).into(),
                formatter: definition.formatter.into(),
                // External availability is intentionally unknown until the
                // user explicitly approves a PATH inspection while formatting.
                available: false,
                external: true,
            });
        }
    }
    capabilities.sort_by(|left, right| {
        left.language
            .cmp(&right.language)
            .then_with(|| left.formatter.cmp(&right.formatter))
    });
    capabilities
}

pub(super) async fn confirm_formatter_discovery(
    app: &AppHandle,
    source_path: &Path,
    plan: &FormatterPlan,
) -> AoneResult<bool> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let message = format!(
        "Aone IDE can look for an optional local formatter before formatting this file.\n\nFormatter: {}\nExecutable name: {}\nFile: {:?}\n\nApproval permits this one PATH inspection only. If the executable is found, Aone will show its exact resolved path and arguments in a second confirmation before any process starts. Workspace configuration and plugins are not read during this discovery step.",
        plan.formatter, plan.executable, source_path,
    );
    app.dialog()
        .message(message)
        .title("Allow formatter discovery")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Inspect PATH".into(),
            "Use built-in only".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    match receiver.await {
        Ok(confirmed) => Ok(confirmed),
        Err(_) => Err(AoneError::Task(
            "formatter discovery dialog closed unexpectedly".into(),
        )),
    }
}

pub(super) fn resolve_formatter(executable: &str) -> Option<ResolvedFormatter> {
    let search_path = env::var_os("PATH")?;
    for directory in env::split_paths(&search_path).filter(|path| path.is_absolute()) {
        let Ok(directory) = directory.canonicalize() else {
            continue;
        };
        let launch_path = directory.join(executable);
        if !launch_path.is_file() {
            continue;
        }
        let Ok(target) = launch_path.canonicalize() else {
            continue;
        };
        let Ok(metadata) = target.metadata() else {
            continue;
        };
        if !metadata.is_file() || !is_executable(&metadata) {
            continue;
        }
        return Some(ResolvedFormatter {
            launch_path,
            target,
            identity: FileIdentity::capture(&metadata),
        });
    }
    None
}

pub(super) async fn run_external_formatter(
    app: &AppHandle,
    workspace_root: &Path,
    source_path: &Path,
    content: &str,
    plan: &FormatterPlan,
    resolved: &ResolvedFormatter,
) -> AoneResult<(ExternalFormatOutcome, Option<String>)> {
    let confirmed = confirm_formatter(app, source_path, plan, resolved).await?;
    if !confirmed {
        return Err(AoneError::InvalidRequest(
            "external formatting was cancelled by the user".into(),
        ));
    }
    if revalidate_formatter(resolved).is_err() {
        return Ok((ExternalFormatOutcome::Failed, None));
    }
    let formatted = execute_formatter(workspace_root, content, plan, resolved).await;
    match formatted {
        Ok(content) if validate_source_content(&content).is_ok() => {
            Ok((ExternalFormatOutcome::Formatted, Some(content)))
        }
        _ => Ok((ExternalFormatOutcome::Failed, None)),
    }
}

async fn confirm_formatter(
    app: &AppHandle,
    source_path: &Path,
    plan: &FormatterPlan,
    resolved: &ResolvedFormatter,
) -> AoneResult<bool> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let args = plan
        .args
        .iter()
        .map(|argument| format!("{argument:?}"))
        .collect::<Vec<_>>()
        .join(" ");
    let message = format!(
        "Aone will run a local formatter.\n\nFormatter: {}\nExecutable launch path: {:?}\nResolved executable target: {:?}\nFile: {:?}\nArguments: [{}]\n\nSource text is sent only over stdin and is not shown here. The formatter may load workspace configuration or plugins.",
        plan.formatter, resolved.launch_path, resolved.target, source_path, args,
    );
    app.dialog()
        .message(message)
        .title("Confirm local formatter")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Format document".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    receiver
        .await
        .map_err(|_| AoneError::Task("formatter confirmation dialog closed unexpectedly".into()))
}

async fn execute_formatter(
    workspace_root: &Path,
    content: &str,
    plan: &FormatterPlan,
    resolved: &ResolvedFormatter,
) -> AoneResult<String> {
    let mut command = Command::new(&resolved.target);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.as_std_mut().arg0(resolved.launch_path.as_os_str());
    }
    command
        .args(&plan.args)
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    for name in SAFE_PARENT_ENV {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| AoneError::Task(format!("failed to start formatter: {error}")))?;
    let process_id = child.id();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AoneError::Task("formatter stdin is unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AoneError::Task("formatter stdout is unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AoneError::Task("formatter stderr is unavailable".into()))?;

    let execution = async {
        let write = async {
            stdin.write_all(content.as_bytes()).await?;
            stdin.shutdown().await
        };
        let read_stdout = read_bounded(stdout, MAX_EDITABLE_FILE_BYTES);
        let read_stderr = read_bounded(stderr, MAX_FORMATTER_STDERR_BYTES);
        let (write, stdout, stderr, status) =
            tokio::join!(write, read_stdout, read_stderr, child.wait());
        write?;
        let stdout = stdout?;
        let _stderr = stderr?;
        let status = status?;
        if !status.success() {
            return Err(std::io::Error::other("formatter exited unsuccessfully"));
        }
        Ok::<_, std::io::Error>(stdout)
    };
    let bytes = match tokio::time::timeout(FORMAT_TIMEOUT, execution).await {
        Ok(result) => {
            result.map_err(|error| AoneError::Task(format!("formatter failed: {error}")))?
        }
        Err(_) => {
            stop_formatter_process_tree(&mut child, process_id).await;
            return Err(AoneError::Task(
                "formatter exceeded the 10-second time limit".into(),
            ));
        }
    };
    String::from_utf8(bytes)
        .map_err(|_| AoneError::InvalidRequest("formatter returned non-UTF-8 output".into()))
}

async fn stop_formatter_process_tree(child: &mut tokio::process::Child, process_id: Option<u32>) {
    #[cfg(unix)]
    if let Some(process_id) = process_id.and_then(|value| i32::try_from(value).ok()) {
        // The formatter is its own process-group leader, so this terminates any
        // plugin/config descendants still holding its output pipes open.
        let _ = unsafe { libc::kill(-process_id, libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    let _ = process_id;
    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn read_bounded<R>(reader: R, maximum: usize) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(maximum.min(64 * 1024));
    reader
        .take((maximum as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > maximum {
        return Err(std::io::Error::other("formatter output exceeded its limit"));
    }
    Ok(bytes)
}

fn revalidate_formatter(formatter: &ResolvedFormatter) -> AoneResult<()> {
    let target = formatter
        .launch_path
        .canonicalize()
        .map_err(|_| AoneError::InvalidRequest("formatter executable no longer exists".into()))?;
    let metadata = target.metadata()?;
    if target != formatter.target || !formatter.identity.matches(&metadata) {
        return Err(AoneError::InvalidRequest(
            "formatter executable changed after confirmation".into(),
        ));
    }
    Ok(())
}

impl FileIdentity {
    fn capture(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt as _;
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            mode: metadata.mode(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        self.length == metadata.len() && self.modified == metadata.modified().ok() && {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                self.device == metadata.dev()
                    && self.inode == metadata.ino()
                    && self.mode == metadata.mode()
                    && self.changed_seconds == metadata.ctime()
                    && self.changed_nanoseconds == metadata.ctime_nsec()
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
    }
}

#[cfg(unix)]
fn is_executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &Metadata) -> bool {
    true
}
