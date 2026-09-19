use serde_json::json;
use tempfile::tempdir;

use super::{
    graph_store::{
        GraphStore, MAX_FILE_LIST_RESULTS, MAX_FILE_SEARCH_QUERY_CHARS, MAX_FILE_SEARCH_RESULTS,
    },
    limits::{GLOBAL_IDENTIFIER_CONFIDENCE, MAX_SEED_NODES, WorkspaceStoreLimits},
};
use crate::analyzer::{analyze_source, language_support};
use crate::{
    domain::{EvidenceKind, GraphQuery, WorkspaceFile},
    scanner::IndexedFile,
};
use std::{collections::HashMap, path::Path};

fn indexed_file(path: &str, source: &str) -> IndexedFile {
    let hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let analysis = analyze_source("workspace", path, source, &hash).unwrap();
    IndexedFile {
        file: WorkspaceFile {
            id: crate::analyzer::stable_id("file", &["workspace", path]),
            relative_path: path.into(),
            language: language_support(Path::new(path)).name.into(),
            capability: language_support(Path::new(path)).capability,
            size_bytes: source.len() as u64,
            content_hash: hash,
            modified_at: "0".into(),
            parse_errors: analysis.parse_errors,
        },
        source: source.into(),
        analysis,
    }
}

#[test]
fn wal_store_replaces_file_facts_atomically() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("src/lib.rs", "fn first() {}"))
        .unwrap();
    let first_counts = store.counts().unwrap();
    store
        .replace_file(&indexed_file("src/lib.rs", "fn second() { call(); }"))
        .unwrap();
    let second_counts = store.counts().unwrap();
    assert_eq!(first_counts.0, 1);
    assert_eq!(second_counts.0, 1);
    assert!(second_counts.1 >= first_counts.1);
    assert_eq!(store.list_files(None, 10).unwrap().len(), 1);
}

#[test]
fn store_accepts_repeated_semantic_definitions_without_id_collisions() {
    let wrappers = (0..9)
        .map(|index| {
            format!(
                "test('case {index}', () => {{\n\
                 const wrapper = ({{ children }}: Props) => <Provider>{{children}}</Provider>;\n\
                 renderHook(() => useCase({index}), {{ wrapper }});\n\
                 }});"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let associated_types = r#"
        impl FromRequestParts<AppState> for AuthUser {
            type Rejection = AppError;
        }
        impl OptionalFromRequestParts<AppState> for AuthUser {
            type Rejection = AppError;
        }
    "#;
    let files = [
        indexed_file("src/workspace-queries.test.tsx", &wrappers),
        indexed_file("src/extractor.rs", associated_types),
    ];
    let expected_node_count = files
        .iter()
        .map(|file| file.analysis.nodes.len())
        .sum::<usize>();
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();

    store.replace_all(&files).unwrap();

    let (file_count, node_count, _) = store.counts().unwrap();
    assert_eq!(file_count, files.len());
    assert_eq!(node_count, expected_node_count);
}

#[test]
fn bounded_file_search_finds_an_indexed_file_beyond_the_initial_tree() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let mut files = (0..MAX_FILE_LIST_RESULTS)
        .map(|index| indexed_file(&format!("src/loaded/{index:05}.txt"), ""))
        .collect::<Vec<_>>();
    let hidden_path = "zzzz/not-in-initial-tree/uniquely-searchable-file.txt";
    files.push(indexed_file(hidden_path, ""));
    store.replace_all(&files).unwrap();

    let initial = store.list_files(None, usize::MAX).unwrap();
    assert_eq!(initial.len(), MAX_FILE_LIST_RESULTS);
    assert!(!initial.iter().any(|file| file.relative_path == hidden_path));

    let result = store
        .list_files(Some("uniquely-searchable"), usize::MAX)
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].relative_path, hidden_path);
    assert_eq!(
        store.list_files(Some(".txt"), usize::MAX).unwrap().len(),
        MAX_FILE_SEARCH_RESULTS
    );
}

#[test]
fn file_search_rejects_oversized_and_control_character_queries() {
    let directory = tempdir().unwrap();
    let store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let oversized = "x".repeat(MAX_FILE_SEARCH_QUERY_CHARS + 1);

    let oversized_error = store.list_files(Some(&oversized), 10).unwrap_err();
    assert!(
        oversized_error
            .to_string()
            .contains("exceeds 256 characters")
    );
    let control_error = store.list_files(Some("src/\nsecret"), 10).unwrap_err();
    assert!(control_error.to_string().contains("control characters"));
}

#[test]
fn graph_queries_are_bounded() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("src/lib.rs", "fn first() { one(); two(); }"))
        .unwrap();
    let snapshot = store
        .query_graph(&GraphQuery {
            limit: Some(2),
            depth: Some(99),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(snapshot.nodes.len() <= 2);
    assert!(snapshot.truncated);
}

#[test]
fn graph_query_text_and_kind_filters_are_bounded_before_sql() {
    let directory = tempdir().unwrap();
    let store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let oversized = store
        .query_graph(&GraphQuery {
            query: Some("x".repeat(257)),
            ..GraphQuery::default()
        })
        .unwrap_err();
    assert!(oversized.to_string().contains("exceeds 256 characters"));
    let control = store
        .query_graph(&GraphQuery {
            query: Some("main\nsecret".into()),
            ..GraphQuery::default()
        })
        .unwrap_err();
    assert!(control.to_string().contains("control characters"));
    let too_many_kinds = store
        .query_graph(&GraphQuery {
            node_kinds: (0..33).map(|index| format!("kind{index}")).collect(),
            ..GraphQuery::default()
        })
        .unwrap_err();
    assert!(too_many_kinds.to_string().contains("nodeKinds exceeds 32"));
    let long_kind = store
        .query_graph(&GraphQuery {
            edge_kinds: vec!["x".repeat(65)],
            ..GraphQuery::default()
        })
        .unwrap_err();
    assert!(long_kind.to_string().contains("edgeKinds entries"));
}

#[test]
fn seed_query_reports_truncation_when_more_matches_exist_than_seed_cap() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let source = (0..130)
        .map(|index| format!("function item{index}() {{}}"))
        .collect::<Vec<_>>()
        .join("\n");
    store
        .replace_file(&indexed_file("src/many.ts", &source))
        .unwrap();

    let snapshot = store
        .query_graph(&GraphQuery {
            limit: Some(250),
            depth: Some(0),
            ..GraphQuery::default()
        })
        .unwrap();

    assert_eq!(snapshot.nodes.len(), MAX_SEED_NODES);
    assert!(snapshot.truncated);
}

#[test]
fn resolver_links_unambiguous_cross_file_calls_imports_and_database_flow() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
            .replace_all(&[
                indexed_file(
                    "src/route.ts",
                    "import { loadUser } from './service'; function route() { router.get('/users', loadUser); loadUser(); }",
                ),
                indexed_file(
                    "src/service.ts",
                    "import { userRepository } from './repository'; export function loadUser() { return userRepository(); }",
                ),
                indexed_file(
                    "src/repository.ts",
                    "export function userRepository() { return prisma.user.findMany(); }",
                ),
            ])
            .unwrap();
    let snapshot = store
        .query_graph(&GraphQuery {
            limit: Some(500),
            depth: Some(4),
            ..GraphQuery::default()
        })
        .unwrap();

    let relative_imports = snapshot
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == "resolvesTo"
                && edge.evidence == EvidenceKind::Resolved
                && edge.metadata.get("resolver") == Some(&json!("relativeImportPath"))
        })
        .collect::<Vec<_>>();
    assert_eq!(relative_imports.len(), 2, "imports: {relative_imports:#?}");

    let inferred_calls = snapshot
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == "resolvesTo"
                && edge.evidence == EvidenceKind::Inferred
                && edge.metadata.get("resolver") == Some(&json!("globalExactFinalIdentifier"))
        })
        .collect::<Vec<_>>();
    assert!(inferred_calls.len() >= 2, "calls: {inferred_calls:#?}");
    assert!(inferred_calls.iter().all(|edge| {
        edge.confidence == Some(GLOBAL_IDENTIFIER_CONFIDENCE)
            && edge.metadata.get("matchScope") == Some(&json!("workspaceGlobal"))
            && edge.metadata.contains_key("basis")
    }));
    assert!(snapshot.nodes.iter().any(|node| node.kind == "endpoint"));
    assert!(snapshot.nodes.iter().any(|node| node.kind == "database"));

    let labels = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.label.as_str()))
        .collect::<HashMap<_, _>>();
    assert!(
        inferred_calls
            .iter()
            .any(|edge| labels.get(edge.target.as_str()) == Some(&"loadUser"))
    );
    assert!(
        inferred_calls
            .iter()
            .any(|edge| labels.get(edge.target.as_str()) == Some(&"userRepository"))
    );
}

#[test]
fn inferred_identifier_links_are_rebuilt_when_a_match_becomes_ambiguous() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("src/caller.ts", "function callIt() { loadUser(); }"),
            indexed_file("src/first.ts", "export function loadUser() {}"),
        ])
        .unwrap();

    let first = store
        .query_graph(&GraphQuery {
            limit: Some(500),
            depth: Some(4),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(first.edges.iter().any(|edge| {
        edge.kind == "resolvesTo"
            && edge.evidence == EvidenceKind::Inferred
            && edge.metadata.get("resolver") == Some(&json!("globalExactFinalIdentifier"))
    }));

    store
        .replace_file(&indexed_file(
            "src/second.ts",
            "export function loadUser() {}",
        ))
        .unwrap();
    let ambiguous = store
        .query_graph(&GraphQuery {
            limit: Some(500),
            depth: Some(4),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(!ambiguous.edges.iter().any(|edge| {
        edge.kind == "resolvesTo"
            && edge.metadata.get("resolver") == Some(&json!("globalExactFinalIdentifier"))
    }));
}

#[test]
fn incremental_limits_roll_back_file_count_overflow() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let first = indexed_file("src/first.rs", "fn first() {}");
    let second = indexed_file("src/second.rs", "fn second() {}");
    let limits = WorkspaceStoreLimits {
        max_files: 1,
        max_source_bytes: u64::MAX,
        max_facts: usize::MAX,
    };
    store.replace_file_with_limits(&first, limits).unwrap();

    let error = store.replace_file_with_limits(&second, limits).unwrap_err();
    assert!(error.to_string().contains("maximum of 1 indexed files"));
    assert_eq!(store.counts().unwrap().0, 1);
    assert!(store.get_file("src/first.rs").unwrap().is_some());
    assert!(store.get_file("src/second.rs").unwrap().is_none());
}

#[test]
fn incremental_limits_roll_back_aggregate_byte_and_fact_overflow() {
    let byte_directory = tempdir().unwrap();
    let mut byte_store = GraphStore::open(&byte_directory.path().join("aone.sqlite")).unwrap();
    let first = indexed_file("src/first.rs", "fn first() {}");
    let second = indexed_file("src/second.rs", "fn second() {}");
    let byte_limits = WorkspaceStoreLimits {
        max_files: 2,
        max_source_bytes: first.file.size_bytes + second.file.size_bytes - 1,
        max_facts: usize::MAX,
    };
    byte_store
        .replace_file_with_limits(&first, byte_limits)
        .unwrap();
    let byte_error = byte_store
        .replace_file_with_limits(&second, byte_limits)
        .unwrap_err();
    assert!(byte_error.to_string().contains("source byte budget"));
    assert_eq!(byte_store.counts().unwrap().0, 1);
    assert!(byte_store.get_file("src/second.rs").unwrap().is_none());

    let fact_directory = tempdir().unwrap();
    let mut fact_store = GraphStore::open(&fact_directory.path().join("aone.sqlite")).unwrap();
    let fact_limits = WorkspaceStoreLimits {
        max_files: 2,
        max_source_bytes: u64::MAX,
        max_facts: 1,
    };
    fact_store
        .replace_file_with_limits(&first, fact_limits)
        .unwrap();
    let fact_error = fact_store
        .replace_file_with_limits(&second, fact_limits)
        .unwrap_err();
    assert!(fact_error.to_string().contains("extracted fact budget"));
    assert_eq!(fact_store.counts().unwrap().0, 1);
    assert!(fact_store.get_file("src/second.rs").unwrap().is_none());
}
