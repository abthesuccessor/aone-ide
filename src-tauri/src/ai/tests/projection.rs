use super::{SecretState, WORKSPACE_ID, node};
use crate::domain::{GraphProjection, GraphSnapshot};

fn snapshot(id: &str) -> GraphSnapshot {
    GraphSnapshot {
        nodes: vec![node(id)],
        edges: Vec::new(),
        truncated: false,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    }
}

#[test]
fn graph_evidence_projection_is_bound_to_its_workspace() {
    let secrets = SecretState::new();
    secrets.replace_graph_nodes("workspace:first", vec![node("selected")]);
    let ids = vec!["selected".into()];
    assert_eq!(
        secrets
            .selected_nodes("workspace:first", &ids, 1)
            .unwrap()
            .len(),
        1
    );
    let error = secrets
        .selected_nodes("workspace:second", &ids, 1)
        .unwrap_err();
    assert!(error.to_string().contains("current workspace"));
}

#[test]
fn system_and_neighborhood_query_evidence_coexist_regardless_of_completion_order() {
    let secrets = SecretState::new();
    let system = snapshot("system");
    let neighborhood = snapshot("detail");
    secrets.replace_query_snapshot(WORKSPACE_ID, GraphProjection::Neighborhood, &neighborhood);
    secrets.replace_query_snapshot(WORKSPACE_ID, GraphProjection::SystemOverview, &system);

    let ids = vec!["system".into(), "detail".into()];
    assert_eq!(
        secrets
            .selected_nodes(WORKSPACE_ID, &ids, ids.len())
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        ids
    );
    secrets.replace_query_snapshot(WORKSPACE_ID, GraphProjection::SystemOverview, &system);
    secrets.replace_query_snapshot(WORKSPACE_ID, GraphProjection::Neighborhood, &neighborhood);
    assert_eq!(
        secrets
            .selected_nodes(WORKSPACE_ID, &ids, ids.len())
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn execution_flow_queries_do_not_replace_ai_evidence_projections() {
    let secrets = SecretState::new();
    secrets.replace_query_snapshot(
        WORKSPACE_ID,
        GraphProjection::Neighborhood,
        &snapshot("detail"),
    );
    secrets.replace_query_snapshot(
        WORKSPACE_ID,
        GraphProjection::SystemOverview,
        &snapshot("system"),
    );
    secrets.replace_query_snapshot(
        WORKSPACE_ID,
        GraphProjection::ExecutionFlow,
        &snapshot("flow"),
    );

    let ids = vec!["system".into(), "detail".into(), "flow".into()];
    let selected = secrets
        .selected_nodes(WORKSPACE_ID, &ids, ids.len())
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert_eq!(selected, vec!["system", "detail"]);
}
