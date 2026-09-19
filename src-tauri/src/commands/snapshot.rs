use tauri::State;

use crate::{domain::AppSnapshot, error::AoneResult, runner::RuntimeState, state::AppState};

#[tauri::command]
pub fn get_app_snapshot(
    state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<AppSnapshot> {
    Ok(AppSnapshot {
        version: env!("CARGO_PKG_VERSION").into(),
        workspace: state
            .workspace_if_open()
            .map(|context| context.summary.read().clone()),
        runtime_event_count: runtime.list_events(Some(2_000)).len(),
    })
}
