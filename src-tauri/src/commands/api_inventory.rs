use tauri::State;

use crate::{
    domain::{ApiEndpointPage, ListApiEndpointsRequest},
    error::{AoneError, AoneResult},
    state::AppState,
};

#[tauri::command]
pub fn list_api_endpoints(
    request: ListApiEndpointsRequest,
    state: State<'_, AppState>,
) -> AoneResult<ApiEndpointPage> {
    validate_workspace_id(&request.workspace_id)?;
    let context = state.workspace()?;
    if context.id != request.workspace_id {
        return Err(AoneError::InvalidRequest(
            "workspaceId does not match the open workspace".into(),
        ));
    }
    let page = context.store.lock().list_api_endpoints(&request)?;
    state.while_workspace_current(&request.workspace_id, || Ok(page))
}

fn validate_workspace_id(workspace_id: &str) -> AoneResult<()> {
    if workspace_id.is_empty()
        || workspace_id.chars().count() > 200
        || workspace_id.chars().any(char::is_control)
    {
        return Err(AoneError::InvalidRequest(
            "workspaceId must contain 1 to 200 non-control characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn validate_workspace_id_for_test(workspace_id: &str) -> AoneResult<()> {
    validate_workspace_id(workspace_id)
}
