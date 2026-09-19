use std::{collections::BTreeMap, time::Instant};

use tempfile::tempdir;

use super::{
    GraphStore,
    bulk_persistence::{bulk_prepare_passes, reset_bulk_prepare_passes},
};
use crate::{
    analyzer::AnalyzedSource,
    domain::{CapabilityLevel, EvidenceKind, GraphEdge, GraphNode, SourceLocation, WorkspaceFile},
    scanner::IndexedFile,
};

fn generated_files(file_count: usize, facts_per_file: usize) -> Vec<IndexedFile> {
    (0..file_count)
        .map(|file_index| generated_file(file_index, facts_per_file))
        .collect()
}

fn generated_file(file_index: usize, facts_per_file: usize) -> IndexedFile {
    let relative_path = format!("src/generated/{file_index:05}.ts");
    let file_node_id = format!("file-{file_index}");
    let mut nodes = vec![node(
        &file_node_id,
        "file",
        &relative_path,
        &relative_path,
        1,
    )];
    let mut edges = Vec::with_capacity(facts_per_file * 3);
    for fact_index in 0..facts_per_file {
        let function_id = format!("function-{file_index}-{fact_index}");
        let call_id = format!("call-{file_index}-{fact_index}");
        let module_id = format!("module-{file_index}-{fact_index}");
        let label = format!("function_{file_index}_{fact_index}");
        nodes.push(node(
            &function_id,
            "function",
            &label,
            &relative_path,
            fact_index + 2,
        ));
        nodes.push(node(
            &call_id,
            "callTarget",
            &format!("{label}()"),
            &relative_path,
            fact_index + 2,
        ));
        nodes.push(node(
            &module_id,
            "module",
            &format!("import './module-{fact_index}'"),
            &relative_path,
            fact_index + 2,
        ));
        for (edge_index, target) in [&function_id, &call_id, &module_id].into_iter().enumerate() {
            edges.push(GraphEdge {
                id: format!("edge-{file_index}-{fact_index}-{edge_index}"),
                source: file_node_id.clone(),
                target: target.clone(),
                kind: "contains".into(),
                evidence: EvidenceKind::Declared,
                confidence: None,
                metadata: BTreeMap::new(),
            });
        }
    }
    let source = format!(
        "// generated persistence corpus {file_index}\n{}",
        "const indexed_search_token = 'workspace graph event';\n".repeat(80)
    );
    let content_hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let analysis = AnalyzedSource {
        fact_count: nodes.len() + edges.len(),
        nodes,
        edges,
        parse_errors: false,
    };
    IndexedFile {
        file: WorkspaceFile {
            id: format!("workspace-file-{file_index}"),
            relative_path,
            language: "TypeScript".into(),
            capability: CapabilityLevel::Semantic,
            size_bytes: source.len() as u64,
            content_hash,
            modified_at: "0".into(),
            parse_errors: false,
        },
        source,
        analysis,
    }
}

fn node(id: &str, kind: &str, label: &str, relative_path: &str, line: usize) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: kind.into(),
        label: label.into(),
        source: Some(SourceLocation {
            relative_path: relative_path.into(),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: label.encode_utf16().count() + 1,
        }),
        language: Some("TypeScript".into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::new(),
    }
}

fn schema_object_count(store: &GraphStore, object_type: &str, names: &[&str]) -> usize {
    let placeholders = std::iter::repeat_n("?", names.len())
        .collect::<Vec<_>>()
        .join(", ");
    store
        .connection
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = ? AND name IN ({placeholders})"
            ),
            rusqlite::params_from_iter(std::iter::once(object_type).chain(names.iter().copied())),
            |row| row.get::<_, i64>(0),
        )
        .unwrap() as usize
}

fn table_count(store: &GraphStore, table: &str) -> usize {
    store
        .connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap() as usize
}

#[test]
fn bulk_replace_prepares_each_row_shape_once() {
    let directory = tempdir().unwrap();
    let files = generated_files(40, 3);
    let mut store = GraphStore::open(&directory.path().join("prepared.sqlite")).unwrap();
    reset_bulk_prepare_passes();

    store.replace_all(&files).unwrap();

    assert_eq!(bulk_prepare_passes(), 1);
    assert_eq!(store.counts().unwrap().0, files.len());
}

#[test]
fn bulk_replace_preserves_search_and_incremental_triggers() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("search.sqlite")).unwrap();
    store.replace_all(&generated_files(4, 2)).unwrap();

    let source = store.search_candidates("indexed_search_token").unwrap();
    assert_eq!(source.source_files.len(), 4);
    let semantic = store.search_candidates("function_0_0").unwrap();
    assert!(
        semantic
            .semantic_nodes
            .iter()
            .any(|candidate| candidate.label == "function_0_0")
    );
    assert_eq!(table_count(&store, "source_search"), 4);
    assert_eq!(table_count(&store, "graph_search"), 8);

    let mut replacement = generated_file(0, 2);
    replacement.source = "const incremental_only_token = true;".into();
    replacement.file.size_bytes = replacement.source.len() as u64;
    replacement.file.content_hash = blake3::hash(replacement.source.as_bytes())
        .to_hex()
        .to_string();
    replacement.analysis.nodes[1].label = "incrementalSemanticLabel".into();
    store.replace_file(&replacement).unwrap();

    let incremental_source = store.search_candidates("incremental_only_token").unwrap();
    assert_eq!(incremental_source.source_files.len(), 1);
    let incremental_semantic = store.search_candidates("incrementalSemanticLabel").unwrap();
    assert!(
        incremental_semantic
            .semantic_nodes
            .iter()
            .any(|candidate| candidate.label == "incrementalSemanticLabel")
    );
    assert_eq!(table_count(&store, "source_search"), 4);
    assert_eq!(table_count(&store, "graph_search"), 8);
    assert_eq!(
        schema_object_count(
            &store,
            "trigger",
            &[
                "search_documents_ad",
                "semantic_search_documents_ad",
                "graph_nodes_ai",
                "graph_nodes_au",
            ],
        ),
        4
    );
    assert_eq!(
        schema_object_count(
            &store,
            "index",
            &[
                "idx_files_language",
                "idx_search_documents_path",
                "idx_nodes_file",
                "idx_nodes_kind_label",
                "idx_edges_source",
                "idx_edges_target",
                "idx_edges_kind",
                "idx_semantic_search_node",
            ],
        ),
        8
    );

    let replacement_path = replacement.file.relative_path.clone();
    store.remove_file(&replacement_path).unwrap();
    assert!(store.get_file(&replacement_path).unwrap().is_none());
    assert!(
        store
            .search_candidates("incremental_only_token")
            .unwrap()
            .source_files
            .is_empty()
    );
    assert!(
        store
            .search_candidates("incrementalSemanticLabel")
            .unwrap()
            .semantic_nodes
            .is_empty()
    );
    assert_eq!(table_count(&store, "source_search"), 3);
    assert_eq!(table_count(&store, "graph_search"), 6);
}

#[test]
fn failed_bulk_replace_rolls_back_rows_indexes_and_triggers() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("rollback.sqlite")).unwrap();
    let original = generated_files(3, 2);
    store.replace_all(&original).unwrap();
    let original_counts = store.counts().unwrap();
    let mut invalid = generated_files(2, 2);
    invalid[1].analysis.nodes[1].id = invalid[0].analysis.nodes[1].id.clone();

    assert!(store.replace_all(&invalid).is_err());
    assert_eq!(store.counts().unwrap(), original_counts);
    assert_eq!(
        store
            .search_candidates("indexed_search_token")
            .unwrap()
            .source_files
            .len(),
        original.len()
    );

    let incremental = generated_file(99, 1);
    store.replace_file(&incremental).unwrap();
    assert!(
        store
            .get_file(&incremental.file.relative_path)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        schema_object_count(
            &store,
            "trigger",
            &[
                "search_documents_ad",
                "semantic_search_documents_ad",
                "graph_nodes_ai",
                "graph_nodes_au",
            ],
        ),
        4
    );
}

#[test]
#[ignore = "manual persistence benchmark; has no timing assertion"]
fn benchmark_representative_full_replace() {
    let directory = tempdir().unwrap();
    let files = generated_files(500, 12);
    let expected_nodes = files.iter().map(|file| file.analysis.nodes.len()).sum();
    let expected_edges = files
        .iter()
        .map(|file| file.analysis.edges.len())
        .sum::<usize>()
        + 6_000;
    let mut store = GraphStore::open(&directory.path().join("benchmark.sqlite")).unwrap();

    let initial_started = Instant::now();
    store.replace_all(&files).unwrap();
    let initial_elapsed = initial_started.elapsed();
    let replacement_started = Instant::now();
    store.replace_all(&files).unwrap();
    let replacement_elapsed = replacement_started.elapsed();

    assert_eq!(
        store.counts().unwrap(),
        (500, expected_nodes, expected_edges)
    );
    eprintln!(
        "full replace: {} files, {expected_nodes} nodes, {expected_edges} edges; initial {initial_elapsed:?}, replacement {replacement_elapsed:?}",
        files.len()
    );
}
