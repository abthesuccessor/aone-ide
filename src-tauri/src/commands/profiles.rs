use tauri::State;

use crate::{
    domain::RunProfile,
    error::{AoneError, AoneResult},
    project_environment::detected_environment_names,
    run_profiles::{attach_required_env, detect_profiles},
    runner::RuntimeState,
    state::AppState,
};

#[tauri::command]
pub fn detect_run_profiles(
    state: State<'_, AppState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<Vec<RunProfile>> {
    let context = state.workspace()?;
    let mut profiles = detect_profiles(&context.root, &context.id)?;
    // Discovery names the command; the project's own configuration names the
    // variables it needs. Joining them here is what lets the pre-flight check
    // warn before a run starts instead of after it fails. A detection failure
    // must not block running, so an unreadable config degrades to no
    // requirements rather than to an error.
    if let Ok(names) = detected_environment_names(&context.root, &context.store.lock()) {
        attach_required_env(&mut profiles, &names);
    }
    runtime
        .replace_profiles_for_workspace(&context.root, profiles.clone())
        .map_err(AoneError::InvalidRequest)?;
    Ok(profiles)
}
