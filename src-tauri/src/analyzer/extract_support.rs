use std::{collections::BTreeMap, rc::Rc};

use tree_sitter::Node;

use crate::domain::{EvidenceKind, GraphEdge};

use super::ids::stable_id;

pub(super) struct VisitFrame<'tree> {
    pub(super) node: Node<'tree>,
    pub(super) owner_id: Rc<str>,
    pub(super) scope: Rc<str>,
    pub(super) sibling_ordinal: usize,
    pub(super) depth: usize,
}

pub(super) fn graph_edge(
    source: &str,
    target: &str,
    kind: &str,
    evidence: EvidenceKind,
    confidence: Option<f64>,
) -> GraphEdge {
    GraphEdge {
        id: stable_id("edge", &[source, target, kind]),
        source: source.into(),
        target: target.into(),
        kind: kind.into(),
        evidence,
        confidence,
        metadata: BTreeMap::new(),
    }
}
