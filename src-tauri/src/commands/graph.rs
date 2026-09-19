use std::collections::HashMap;

use serde_json::{Value, json};
use tauri::State;

use crate::{
    ai::SecretState,
    domain::{GraphProjection, GraphQuery, GraphSnapshot, RuntimeEvent},
    error::AoneResult,
    runner::RuntimeState,
    state::{AppState, WorkspaceContext},
};

#[tauri::command]
pub fn query_graph(
    query: GraphQuery,
    state: State<'_, AppState>,
    secrets: State<'_, SecretState>,
    runtime: State<'_, RuntimeState>,
) -> AoneResult<GraphSnapshot> {
    let context = state.workspace()?;
    let events = (query.projection == GraphProjection::ExecutionFlow)
        .then(|| runtime.list_events_for_workspace(&context.root, Some(2_000)))
        .transpose()?
        .unwrap_or_default();
    let mut snapshot = context.store.lock().query_graph(&query)?;
    if query.projection == GraphProjection::ExecutionFlow {
        apply_runtime_overlay(&mut snapshot, &events);
    }
    state.while_workspace_current(&context.id, || {
        secrets.replace_query_snapshot(&context.id, query.projection, &snapshot);
        Ok(())
    })?;
    Ok(snapshot)
}

pub(super) fn apply_runtime_overlay(snapshot: &mut GraphSnapshot, events: &[RuntimeEvent]) {
    let mut by_node = HashMap::<&str, Vec<&RuntimeEvent>>::new();
    for event in events {
        if let Some(node_id) = event.source_node_id.as_deref() {
            by_node.entry(node_id).or_default().push(event);
        }
    }
    for node in &mut snapshot.nodes {
        let Some(matches) = by_node.get(node.id.as_str()) else {
            continue;
        };
        let mut event_ids = matches
            .iter()
            .rev()
            .take(20)
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>();
        event_ids.reverse();
        let mut trace_ids = matches
            .iter()
            .filter_map(|event| event.trace_id.as_deref())
            .collect::<Vec<_>>();
        trace_ids.sort_unstable();
        trace_ids.dedup();
        trace_ids.truncate(20);
        node.metadata
            .insert("observedEventCount".into(), json!(matches.len()));
        node.metadata
            .insert("observedEventIds".into(), json!(event_ids));
        node.metadata
            .insert("observedTraceIds".into(), json!(trace_ids));
        if let Some(timestamp) = matches.iter().map(|event| event.timestamp.as_str()).max() {
            node.metadata
                .insert("lastObservedAt".into(), Value::String(timestamp.into()));
        }
        node.metadata.insert("flowStaticOnly".into(), json!(false));
        node.metadata.insert(
            "runtimeOverlayBasis".into(),
            json!("backend-authored exact sourceNodeId only"),
        );
    }
}

pub(super) fn sync_graph_to_ai(
    state: &AppState,
    context: &WorkspaceContext,
    secrets: &SecretState,
) -> AoneResult<()> {
    let snapshot = context.store.lock().query_graph(&GraphQuery {
        limit: Some(500),
        depth: Some(1),
        ..GraphQuery::default()
    })?;
    state.while_workspace_current(&context.id, || {
        secrets.replace_graph_snapshot(&context.id, &snapshot);
        Ok(())
    })
}
