use std::collections::HashMap;

#[cfg(test)]
use petgraph::algo::astar;
use petgraph::{algo::kosaraju_scc, graph::DiGraph};

use crate::domain::GraphSnapshot;

pub fn strongly_connected_components(snapshot: &GraphSnapshot) -> Vec<Vec<String>> {
    let (graph, _) = projection(snapshot);
    let mut components = kosaraju_scc(&graph)
        .into_iter()
        .map(|component| {
            let mut ids = component
                .into_iter()
                .map(|index| graph[index].clone())
                .collect::<Vec<_>>();
            ids.sort();
            ids
        })
        .filter(|component| component.len() > 1)
        .collect::<Vec<_>>();
    components.sort_by(|left, right| left[0].cmp(&right[0]));
    components
}

#[cfg(test)]
pub fn shortest_path(snapshot: &GraphSnapshot, source: &str, target: &str) -> Option<Vec<String>> {
    let (graph, indexes) = projection(snapshot);
    let source = *indexes.get(source)?;
    let target = *indexes.get(target)?;
    astar(&graph, source, |node| node == target, |_| 1_u32, |_| 0_u32)
        .map(|(_, path)| path.into_iter().map(|index| graph[index].clone()).collect())
}

fn projection(
    snapshot: &GraphSnapshot,
) -> (
    DiGraph<String, ()>,
    HashMap<String, petgraph::graph::NodeIndex>,
) {
    let mut graph = DiGraph::<String, ()>::new();
    let mut indexes = HashMap::new();
    for node in &snapshot.nodes {
        let index = graph.add_node(node.id.clone());
        indexes.insert(node.id.clone(), index);
    }
    for edge in &snapshot.edges {
        if let (Some(source), Some(target)) = (indexes.get(&edge.source), indexes.get(&edge.target))
        {
            graph.add_edge(*source, *target, ());
        }
    }
    (graph, indexes)
}
