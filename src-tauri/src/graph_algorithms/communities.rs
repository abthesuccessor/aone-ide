use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::json;

use crate::{analyzer::stable_id, domain::GraphSnapshot};

const MAX_LABEL_PROPAGATION_ROUNDS: usize = 64;
const COMMUNITY_ALGORITHM: &str = "deterministicLabelPropagationV1";
const COMMUNITY_BASIS: &str = "boundedGraphSnapshot";

pub fn annotate_structural_communities(snapshot: &mut GraphSnapshot) {
    let adjacency = adjacency(snapshot);
    let mut labels = adjacency
        .keys()
        .map(|node_id| (node_id.clone(), node_id.clone()))
        .collect::<BTreeMap<_, _>>();

    for _ in 0..MAX_LABEL_PROPAGATION_ROUNDS {
        let mut changed = false;
        for (node_id, neighbors) in &adjacency {
            if neighbors.is_empty() {
                continue;
            }
            let mut scores = BTreeMap::<String, usize>::new();
            for neighbor_id in neighbors {
                let Some(label) = labels.get(neighbor_id) else {
                    continue;
                };
                let weight = structural_weight(node_id, neighbor_id, &adjacency);
                let score = scores.entry(label.clone()).or_default();
                *score = score.saturating_add(weight);
            }
            let next_label = scores
                .into_iter()
                .max_by(|(left_label, left_score), (right_label, right_score)| {
                    left_score
                        .cmp(right_score)
                        .then_with(|| right_label.cmp(left_label))
                })
                .map(|(label, _)| label);
            if let Some(next_label) = next_label
                && labels.get(node_id) != Some(&next_label)
            {
                labels.insert(node_id.clone(), next_label);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut members_by_label = BTreeMap::<String, Vec<String>>::new();
    for (node_id, label) in labels {
        members_by_label.entry(label).or_default().push(node_id);
    }
    let mut assignment = HashMap::<String, (String, usize)>::new();
    for members in members_by_label.values_mut() {
        members.sort();
        let member_refs = members.iter().map(String::as_str).collect::<Vec<_>>();
        let community_id = stable_id("community", &member_refs);
        let community_size = members.len();
        for member in members {
            assignment.insert(member.clone(), (community_id.clone(), community_size));
        }
    }

    let complete = !snapshot.truncated;
    for node in &mut snapshot.nodes {
        let Some((community_id, community_size)) = assignment.get(&node.id) else {
            continue;
        };
        node.metadata
            .insert("communityId".into(), json!(community_id));
        node.metadata
            .insert("communitySize".into(), json!(community_size));
        node.metadata
            .insert("communityAlgorithm".into(), json!(COMMUNITY_ALGORITHM));
        node.metadata
            .insert("communityBasis".into(), json!(COMMUNITY_BASIS));
        node.metadata
            .insert("communityComplete".into(), json!(complete));
    }
}

fn adjacency(snapshot: &GraphSnapshot) -> BTreeMap<String, BTreeSet<String>> {
    let mut adjacency = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in &snapshot.edges {
        if edge.source == edge.target
            || !adjacency.contains_key(&edge.source)
            || !adjacency.contains_key(&edge.target)
        {
            continue;
        }
        adjacency
            .get_mut(&edge.source)
            .expect("validated source")
            .insert(edge.target.clone());
        adjacency
            .get_mut(&edge.target)
            .expect("validated target")
            .insert(edge.source.clone());
    }
    adjacency
}

fn structural_weight(
    source: &str,
    target: &str,
    adjacency: &BTreeMap<String, BTreeSet<String>>,
) -> usize {
    let Some(source_neighbors) = adjacency.get(source) else {
        return 1;
    };
    let Some(target_neighbors) = adjacency.get(target) else {
        return 1;
    };
    1_usize.saturating_add(source_neighbors.intersection(target_neighbors).count())
}
