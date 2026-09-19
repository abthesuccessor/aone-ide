use std::collections::{HashMap, HashSet};

use crate::domain::{GraphEdge, GraphNode, GraphProjection, GraphSnapshot};

/// Bounded, backend-owned graph evidence that can be cited by an AI request.
pub(super) struct WorkspaceGraphProjection {
    pub(super) workspace_id: String,
    system_nodes: HashMap<String, GraphNode>,
    system_edges: HashMap<String, GraphEdge>,
    neighborhood_nodes: HashMap<String, GraphNode>,
    neighborhood_edges: HashMap<String, GraphEdge>,
}

impl WorkspaceGraphProjection {
    pub(super) fn from_snapshot(workspace_id: &str, snapshot: &GraphSnapshot) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            system_nodes: HashMap::new(),
            system_edges: HashMap::new(),
            neighborhood_nodes: node_map(snapshot),
            neighborhood_edges: edge_map(snapshot),
        }
    }

    pub(super) fn from_nodes(workspace_id: &str, nodes: Vec<GraphNode>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            system_nodes: HashMap::new(),
            system_edges: HashMap::new(),
            neighborhood_nodes: nodes
                .into_iter()
                .map(|node| (node.id.clone(), node))
                .collect(),
            neighborhood_edges: HashMap::new(),
        }
    }

    pub(super) fn empty(workspace_id: &str) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            system_nodes: HashMap::new(),
            system_edges: HashMap::new(),
            neighborhood_nodes: HashMap::new(),
            neighborhood_edges: HashMap::new(),
        }
    }

    pub(super) fn replace(&mut self, kind: GraphProjection, snapshot: &GraphSnapshot) {
        let (nodes, edges) = match kind {
            GraphProjection::Neighborhood => {
                (&mut self.neighborhood_nodes, &mut self.neighborhood_edges)
            }
            GraphProjection::SystemOverview => (&mut self.system_nodes, &mut self.system_edges),
            GraphProjection::ExecutionFlow => return,
        };
        *nodes = node_map(snapshot);
        *edges = edge_map(snapshot);
    }

    pub(super) fn selected_nodes(&self, ids: &[String], max: usize) -> Vec<GraphNode> {
        ids.iter()
            .take(max)
            .filter_map(|id| {
                self.system_nodes
                    .get(id)
                    .or_else(|| self.neighborhood_nodes.get(id))
                    .cloned()
            })
            .collect()
    }

    pub(super) fn selected_edges(&self, node_ids: &[String], max: usize) -> Vec<GraphEdge> {
        let selected = node_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        let mut edges = self
            .system_edges
            .values()
            .chain(self.neighborhood_edges.values())
            .filter(|edge| {
                selected.contains(edge.source.as_str()) && selected.contains(edge.target.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        edges.dedup_by(|left, right| left.id == right.id);
        edges.truncate(max);
        edges
    }
}

fn node_map(snapshot: &GraphSnapshot) -> HashMap<String, GraphNode> {
    snapshot
        .nodes
        .iter()
        .cloned()
        .map(|node| (node.id.clone(), node))
        .collect()
}

fn edge_map(snapshot: &GraphSnapshot) -> HashMap<String, GraphEdge> {
    snapshot
        .edges
        .iter()
        .cloned()
        .map(|edge| (edge.id.clone(), edge))
        .collect()
}
