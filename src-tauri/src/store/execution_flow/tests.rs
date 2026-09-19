use std::collections::BTreeMap;

use serde_json::Value;
use tempfile::{TempDir, tempdir};

use crate::{
    analyzer::AnalyzedSource,
    domain::{
        CapabilityLevel, EvidenceKind, GraphEdge, GraphNode, GraphProjection, GraphQuery,
        GraphSnapshot, SourceLocation, WorkspaceFile,
    },
    scanner::IndexedFile,
    store::GraphStore,
};

mod bounds;
mod paging;
mod truth;

fn node(id: &str, kind: &str, label: &str, path: &str, language: &str, line: usize) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: kind.into(),
        label: label.into(),
        source: Some(SourceLocation {
            relative_path: path.into(),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: label.encode_utf16().count().saturating_add(1),
        }),
        language: Some(language.into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::new(),
    }
}

fn edge(id: &str, source: &str, target: &str, kind: &str, evidence: EvidenceKind) -> GraphEdge {
    GraphEdge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        kind: kind.into(),
        evidence,
        confidence: Some(1.0),
        metadata: BTreeMap::new(),
    }
}

fn indexed_file(path: &str, nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> IndexedFile {
    let source = format!("// execution-flow fixture for {path}\n");
    let hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    IndexedFile {
        file: WorkspaceFile {
            id: format!("file:{path}"),
            relative_path: path.into(),
            language: "fixture".into(),
            capability: CapabilityLevel::Semantic,
            size_bytes: source.len() as u64,
            content_hash: hash,
            modified_at: "0".into(),
            parse_errors: false,
        },
        source,
        analysis: AnalyzedSource {
            fact_count: nodes.len().saturating_add(edges.len()),
            nodes,
            edges,
            parse_errors: false,
        },
    }
}

fn open_store(files: &[IndexedFile]) -> (TempDir, GraphStore) {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store.replace_all(files).unwrap();
    (directory, store)
}

fn flow_query(root_id: &str, limit: usize, cursor: Option<String>) -> GraphQuery {
    GraphQuery {
        projection: GraphProjection::ExecutionFlow,
        root_ids: vec![root_id.into()],
        depth: Some(4),
        limit: Some(limit),
        cursor,
        ..GraphQuery::default()
    }
}

fn metadata(node: &mut GraphNode, entries: &[(&str, Value)]) {
    for (key, value) in entries {
        node.metadata.insert((*key).into(), value.clone());
    }
}

fn by_id<'a>(snapshot: &'a GraphSnapshot, id: &str) -> &'a GraphNode {
    snapshot.nodes.iter().find(|node| node.id == id).unwrap()
}
