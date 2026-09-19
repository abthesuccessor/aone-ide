use tauri::State;

use super::state::SecretState;
use crate::domain::AiConfigurationStatus;

/// Returns only renderer-safe AI configuration metadata. Provider credentials
/// remain owned by `SecretState` and are never copied into the response.
#[tauri::command]
pub fn get_ai_configuration_status(secrets: State<'_, SecretState>) -> AiConfigurationStatus {
    secrets.configuration_status()
}
