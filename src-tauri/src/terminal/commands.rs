use std::sync::Arc;

use portable_pty::{CommandBuilder, native_pty_system};
use tauri::{AppHandle, State};
use uuid::Uuid;

use super::{
    confirmation::confirm_terminal_open,
    environment::isolated_shell_command,
    io::spawn_session_io,
    profiles::{capture_directory_identity, find_profile, list_profiles, revalidate_directory},
    pty_io::configure_nonblocking,
    request::{decode_input, pty_size, validate_session_id},
    state::{TerminalSession, TerminalState},
};
use crate::{
    domain::{
        TerminalActionResult, TerminalCloseRequest, TerminalOpenRequest, TerminalOpenResult,
        TerminalProfile, TerminalResizeRequest, TerminalWriteRequest,
    },
    error::{AoneError, AoneResult},
    state::AppState,
};

#[tauri::command]
pub fn list_terminal_profiles() -> Vec<TerminalProfile> {
    list_profiles()
        .into_iter()
        .map(|profile| profile.public)
        .collect()
}

#[tauri::command]
pub async fn open_terminal(
    request: TerminalOpenRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    terminal: State<'_, TerminalState>,
) -> AoneResult<TerminalOpenResult> {
    let registry = terminal.registry();
    let reservation = registry.reserve()?;
    let profile = find_profile(&request.profile_id)?;
    let workspace = app_state.workspace()?;
    let cwd_identity = capture_directory_identity(&workspace.root)?;
    let cwd = workspace.root.canonicalize()?;

    confirm_terminal_open(&app, &profile.public, &cwd).await?;

    // Consent is bound to the filesystem objects, not only their path text.
    let current_workspace = app_state.workspace()?;
    if current_workspace.id != workspace.id || current_workspace.root != workspace.root {
        return Err(AoneError::InvalidRequest(
            "workspace changed while confirming the terminal".into(),
        ));
    }
    profile.revalidate()?;
    let cwd = revalidate_directory(&workspace.root, &cwd_identity)?;
    let size = pty_size(request.columns, request.rows);
    let pair = native_pty_system()
        .openpty(size)
        .map_err(|error| AoneError::Task(format!("failed to allocate terminal: {error}")))?;
    configure_nonblocking(pair.master.as_ref())?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| AoneError::Task(format!("failed to open terminal output: {error}")))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| AoneError::Task(format!("failed to open terminal input: {error}")))?;

    let mut command: CommandBuilder = isolated_shell_command(&profile.shell);
    command.cwd(&cwd);
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| AoneError::Task(format!("failed to start terminal shell: {error}")))?;
    drop(pair.slave);

    let process_group = child_process_group(child.process_id());
    let killer = child.clone_killer();
    let session_id = format!("terminal:{}", Uuid::now_v7());
    let session = Arc::new(TerminalSession::new(
        session_id.clone(),
        writer,
        pair.master,
        killer,
        process_group,
        Arc::downgrade(&registry),
    ));
    let closed = session.closed_signal();
    let workers = session.workers();
    reservation.activate(session)?;
    spawn_session_io(
        app,
        registry,
        session_id.clone(),
        reader,
        child,
        closed,
        workers,
    );

    Ok(TerminalOpenResult {
        session_id,
        profile_id: profile.public.id,
        shell_label: profile.public.label,
        cwd: cwd.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn write_terminal(
    request: TerminalWriteRequest,
    terminal: State<'_, TerminalState>,
) -> AoneResult<TerminalActionResult> {
    validate_session_id(&request.session_id)?;
    let data = decode_input(&request)?;
    let accepted = match terminal.registry().session(&request.session_id) {
        Some(session) => session.write(data)?,
        None => false,
    };
    Ok(TerminalActionResult { accepted })
}

#[tauri::command]
pub fn resize_terminal(
    request: TerminalResizeRequest,
    terminal: State<'_, TerminalState>,
) -> AoneResult<TerminalActionResult> {
    validate_session_id(&request.session_id)?;
    let size = pty_size(request.columns, request.rows);
    let accepted = match terminal.registry().session(&request.session_id) {
        Some(session) => session.resize(size)?,
        None => false,
    };
    Ok(TerminalActionResult { accepted })
}

#[tauri::command]
pub fn close_terminal(
    request: TerminalCloseRequest,
    terminal: State<'_, TerminalState>,
) -> AoneResult<TerminalActionResult> {
    validate_session_id(&request.session_id)?;
    Ok(TerminalActionResult {
        accepted: terminal.registry().close(&request.session_id),
    })
}

#[cfg(unix)]
fn child_process_group(pid: Option<u32>) -> Option<i32> {
    pid.and_then(|value| i32::try_from(value).ok())
}

#[cfg(not(unix))]
fn child_process_group(_pid: Option<u32>) -> Option<i32> {
    None
}
