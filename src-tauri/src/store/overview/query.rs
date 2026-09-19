use std::collections::{HashMap, HashSet};

use rusqlite::params_from_iter;
use serde_json::Value;

use super::{
    candidates::select_diverse_seeds,
    classify::{eligible_node, placement},
};
use crate::{
    domain::{GraphEdge, GraphNode, GraphQuery, GraphSnapshot},
    error::AoneResult,
    graph_algorithms::{annotate_structural_communities, strongly_connected_components},
    store::{
        GraphStore,
        limits::{MAX_GRAPH_DEPTH, MAX_GRAPH_ROOTS},
        records::edge_from_row,
    },
};

const DEFAULT_OVERVIEW_NODES: usize = 60;
const MAX_OVERVIEW_NODES: usize = 60;
const MAX_OVERVIEW_EDGES: usize = 120;
const EDGE_FETCH_LIMIT: usize = MAX_OVERVIEW_EDGES * 2;

pub(in crate::store) fn query_system_overview(
    store: &GraphStore,
    query: &GraphQuery,
) -> AoneResult<GraphSnapshot> {
    let node_limit = query
        .limit
        .unwrap_or(DEFAULT_OVERVIEW_NODES)
        .clamp(1, MAX_OVERVIEW_NODES);
    let depth = query.depth.unwrap_or(2).min(MAX_GRAPH_DEPTH);
    let (seeds, mut truncated) = if query.root_ids.is_empty() {
        select_diverse_seeds(store, query, node_limit)?
    } else {
        load_explicit_seeds(store, query, node_limit)?
    };
    let mut nodes = seeds
        .into_iter()
        .map(|node| (node.id.clone(), node))
        .collect::<HashMap<_, _>>();
    let mut frontier = nodes.keys().cloned().collect::<Vec<_>>();
    let mut edge_map = HashMap::<String, GraphEdge>::new();

    for _ in 0..depth {
        if frontier.is_empty() || nodes.len() >= node_limit || edge_map.len() >= MAX_OVERVIEW_EDGES
        {
            break;
        }
        let (edges, edge_truncated) = overview_edges_touching(store, &frontier, query)?;
        truncated |= edge_truncated;
        let mut candidate_ids = edges
            .iter()
            .flat_map(|edge| [&edge.source, &edge.target])
            .filter(|id| !nodes.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        candidate_ids.sort();
        candidate_ids.dedup();

        let mut candidates = store
            .load_nodes(&candidate_ids)?
            .into_iter()
            .filter(eligible_node)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.id.cmp(&right.id));
        let remaining = node_limit.saturating_sub(nodes.len());
        if candidates.len() > remaining {
            candidates.truncate(remaining);
            truncated = true;
        }
        frontier.clear();
        for node in candidates {
            frontier.push(node.id.clone());
            nodes.insert(node.id.clone(), node);
        }
        for edge in edges {
            if !nodes.contains_key(&edge.source) || !nodes.contains_key(&edge.target) {
                continue;
            }
            if edge_map.len() >= MAX_OVERVIEW_EDGES {
                truncated = true;
                break;
            }
            edge_map.insert(edge.id.clone(), edge);
        }
    }

    let selected = nodes.keys().cloned().collect::<HashSet<_>>();
    let mut edges = edge_map
        .into_values()
        .filter(|edge| selected.contains(&edge.source) && selected.contains(&edge.target))
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    if edges.len() > MAX_OVERVIEW_EDGES {
        edges.truncate(MAX_OVERVIEW_EDGES);
        truncated = true;
    }
    let mut nodes = nodes.into_values().collect::<Vec<_>>();
    decorate_nodes(&mut nodes, &edges);
    nodes.sort_by(|left, right| {
        placement(left)
            .layer
            .cmp(&placement(right).layer)
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut snapshot = GraphSnapshot {
        nodes,
        edges,
        truncated,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    };
    annotate_structural_communities(&mut snapshot);
    Ok(snapshot)
}

fn load_explicit_seeds(
    store: &GraphStore,
    query: &GraphQuery,
    node_limit: usize,
) -> AoneResult<(Vec<GraphNode>, bool)> {
    let root_limit = node_limit.min(MAX_GRAPH_ROOTS);
    let ids = query
        .root_ids
        .iter()
        .take(root_limit)
        .cloned()
        .collect::<Vec<_>>();
    let mut nodes = store
        .load_nodes(&ids)?
        .into_iter()
        .filter(eligible_node)
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let truncated = query.root_ids.len() > root_limit || nodes.len() < ids.len();
    Ok((nodes, truncated))
}

fn overview_edges_touching(
    store: &GraphStore,
    ids: &[String],
    query: &GraphQuery,
) -> AoneResult<(Vec<GraphEdge>, bool)> {
    if ids.is_empty() {
        return Ok((Vec::new(), false));
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut sql = format!(
        "SELECT e.id, e.source, e.target, e.kind, e.evidence, e.confidence, e.metadata_json
         FROM graph_edges e
         JOIN graph_nodes source_node ON source_node.id = e.source
         JOIN graph_nodes target_node ON target_node.id = e.target
         WHERE (e.source IN ({placeholders}) OR e.target IN ({placeholders}))
           AND e.kind IN ('contains','handles','requests','listensTo','emits','queries','accesses','reads','writes','resolvesTo')
           AND NOT (e.kind = 'resolvesTo' AND e.evidence = 'inferred')
           AND source_node.kind NOT IN ('callTarget','module')
           AND target_node.kind NOT IN ('callTarget','module')
           AND {} = 0 AND {} = 0",
        test_path_penalty("source_node.file_path"),
        test_path_penalty("target_node.file_path")
    );
    let mut values = ids.iter().chain(ids.iter()).cloned().collect::<Vec<_>>();
    if !query.edge_kinds.is_empty() {
        sql.push_str(&format!(
            " AND e.kind IN ({})",
            std::iter::repeat_n("?", query.edge_kinds.len())
                .collect::<Vec<_>>()
                .join(",")
        ));
        values.extend(query.edge_kinds.iter().cloned());
    }
    sql.push_str(" ORDER BY e.id LIMIT ?");
    values.push(EDGE_FETCH_LIMIT.saturating_add(1).to_string());
    let mut statement = store.connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), edge_from_row)?;
    let mut edges = rows.collect::<Result<Vec<_>, _>>()?;
    let truncated = edges.len() > EDGE_FETCH_LIMIT;
    edges.truncate(EDGE_FETCH_LIMIT);
    Ok((edges, truncated))
}

fn decorate_nodes(nodes: &mut [GraphNode], edges: &[GraphEdge]) {
    for node in nodes.iter_mut() {
        let role = placement(node);
        node.metadata
            .insert("systemLayer".into(), Value::String(role.layer.id().into()));
        node.metadata
            .insert("systemLayerBasis".into(), Value::String(role.basis));
        node.metadata.insert(
            "systemLayerEvidence".into(),
            Value::String(role.evidence.into()),
        );
    }
    let snapshot = GraphSnapshot {
        nodes: nodes.to_vec(),
        edges: edges.to_vec(),
        truncated: false,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    };
    for (component_index, component) in strongly_connected_components(&snapshot)
        .into_iter()
        .enumerate()
    {
        for node_id in component {
            if let Some(node) = nodes.iter_mut().find(|node| node.id == node_id) {
                node.metadata.insert(
                    "stronglyConnectedComponent".into(),
                    Value::from(component_index as u64),
                );
            }
        }
    }
}

fn test_path_penalty(column: &str) -> String {
    format!(
        "CASE WHEN lower({column}) LIKE '%/tests/%' OR lower({column}) LIKE 'tests/%' OR lower({column}) LIKE '%/test/%' OR lower({column}) LIKE 'test/%' OR lower({column}) LIKE '%/e2e/%' OR lower({column}) LIKE 'e2e/%' OR lower({column}) LIKE '%/__tests__/%' OR lower({column}) LIKE '__tests__/%' OR lower({column}) LIKE '%.test.%' OR lower({column}) LIKE '%.spec.%' OR lower({column}) LIKE '%_test.%' OR lower({column}) LIKE '%_tests.%' OR lower({column}) LIKE '%-test.%' OR lower({column}) LIKE '%-tests.%' THEN 1 ELSE 0 END"
    )
}
