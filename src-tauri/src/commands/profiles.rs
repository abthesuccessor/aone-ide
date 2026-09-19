use tauri::State;

use crate::{
    domain::RunProfile,
    error::{AoneError, AoneResult},
    run_profiles::detect_profiles,
    runner::RuntimeState,
    state::AppState,
};

#[tauri::command]
pub fn detect_run_profiles(
    state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<Vec<RunProfile>> {
    let context = state.workspace()?;
    let profiles = detect_profiles(&context.root, &context.id)?;
    runtime
        .replace_profiles_for_workspace(&context.root, profiles.clone())
        .map_err(AoneError::InvalidRequest)?;
    Ok(profiles)
}
