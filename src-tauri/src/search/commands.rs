use tauri::State;

use crate::{
    domain::{WorkspaceSearchRequest, WorkspaceSearchResult},
    error::{AoneError, AoneResult},
    state::AppState,
};

use super::{
    engine::execute_search,
    limits::{MAX_QUERY_CHARS, MAX_RESULTS},
    state::SearchState,
};

#[tauri::command]
pub async fn search_workspace(
    request: WorkspaceSearchRequest,
    state: State<'_, AppState>,
    search_state: State<'_, SearchState>,
) -> AoneResult<WorkspaceSearchResult> {
    let (query, limit) = validate_request(request)?;
    let context = state.workspace()?;
    if context.id != query.workspace_id {
        return Err(AoneError::InvalidRequest(
            "workspace changed before the search started".into(),
        ));
    }
    let expected_workspace_id = context.id.clone();
    let root = context.root.clone();
    let search_query = query.query.clone();
    let token = search_state.begin();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if !token.is_current() {
            return Ok(WorkspaceSearchResult {
                query: search_query,
                matches: Vec::new(),
                total_matches: 0,
                truncated: true,
                indexed_file_count: 0,
            });
        }
        let candidates = context.store.lock().search_candidates(&search_query)?;
        Ok::<_, AoneError>(execute_search(
            &root,
            &search_query,
            limit,
            candidates,
            token,
        ))
    })
    .await
    .map_err(|error| AoneError::Task(error.to_string()))??;

    state.while_workspace_current(&expected_workspace_id, || Ok(result))
}

struct ValidatedSearch {
    workspace_id: String,
    query: String,
}

fn validate_request(request: WorkspaceSearchRequest) -> AoneResult<(ValidatedSearch, usize)> {
    if request.workspace_id.trim().is_empty()
        || request.workspace_id.chars().count() > 200
        || request.workspace_id.chars().any(char::is_control)
    {
        return Err(AoneError::InvalidRequest(
            "workspaceId must contain 1 to 200 non-control characters".into(),
        ));
    }
    if request.query.chars().any(char::is_control) {
        return Err(AoneError::InvalidRequest(
            "workspace search query cannot contain control characters".into(),
        ));
    }
    let query = request.query.trim();
    if query.is_empty() {
        return Err(AoneError::InvalidRequest(
            "workspace search query cannot be empty".into(),
        ));
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(AoneError::InvalidRequest(format!(
            "workspace search query exceeds {MAX_QUERY_CHARS} characters"
        )));
    }
    if !(1..=MAX_RESULTS).contains(&request.limit) {
        return Err(AoneError::InvalidRequest(format!(
            "workspace search limit must be between 1 and {MAX_RESULTS}"
        )));
    }
    Ok((
        ValidatedSearch {
            workspace_id: request.workspace_id,
            query: query.into(),
        },
        request.limit,
    ))
}

#[cfg(test)]
mod tests {
    use super::validate_request;
    use crate::domain::WorkspaceSearchRequest;

    #[test]
    fn validates_search_boundaries() {
        let valid = validate_request(WorkspaceSearchRequest {
            workspace_id: "workspace".into(),
            query: " listener ".into(),
            limit: 100,
        })
        .unwrap();
        assert_eq!(valid.0.query, "listener");

        for query in ["", "\n"] {
            assert!(
                validate_request(WorkspaceSearchRequest {
                    workspace_id: "workspace".into(),
                    query: query.into(),
                    limit: 10,
                })
                .is_err()
            );
        }
    }
}
