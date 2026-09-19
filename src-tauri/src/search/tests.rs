use std::{fs, path::Path};

use tempfile::tempdir;

use super::{
    engine::{execute_search, execute_search_with_stats},
    limits::MAX_SOURCE_READ_BYTES,
    state::SearchState,
};
use crate::{
    analyzer::{analyze_source, language_support, stable_id},
    domain::{WorkspaceFile, WorkspaceSearchMatchKind},
    scanner::IndexedFile,
    store::{GraphStore, SearchCandidates, SourceSearchCandidate},
};

fn search_token() -> super::state::SearchToken {
    SearchState::new().begin()
}

fn indexed_file(path: &str, source: &str) -> IndexedFile {
    let hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let support = language_support(Path::new(path));
    let analysis = analyze_source("workspace", path, source, &hash).unwrap();
    IndexedFile {
        file: WorkspaceFile {
            id: stable_id("file", &["workspace", path]),
            relative_path: path.into(),
            language: support.name.into(),
            capability: support.capability,
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
fn exact_search_returns_semantic_and_content_locations() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    fs::create_dir(workspace.path().join("src")).unwrap();
    let source = "const app = router();\napp.get('/users', loadUsers);\nfunction loadUsers() {}\n";
    fs::write(workspace.path().join("src/routes.ts"), source).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("src/routes.ts", source))
        .unwrap();

    let candidates = store.search_candidates("users").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let result = execute_search(&root, "users", 50, candidates, search_token());

    assert_eq!(result.indexed_file_count, 1);
    assert!(
        result.matches.iter().any(|found| {
            found.kind == WorkspaceSearchMatchKind::Content
                && found.relative_path == "src/routes.ts"
                && found.start_line == 2
                && found.start_column == 11
                && found.end_column == 16
                && found.match_text == "users"
        }),
        "{result:#?}"
    );
    assert!(result.matches.iter().any(|found| {
        matches!(
            found.kind,
            WorkspaceSearchMatchKind::Endpoint | WorkspaceSearchMatchKind::Symbol
        )
    }));
}

#[test]
fn document_search_preserves_heading_and_sentence_kinds() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    let source = "# Evidence heading\n\nA bounded sentence marker.\n";
    fs::write(workspace.path().join("guide.md"), source).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("guide.md", source))
        .unwrap();
    let root = workspace.path().canonicalize().unwrap();

    let heading = execute_search(
        &root,
        "Evidence heading",
        20,
        store.search_candidates("Evidence heading").unwrap(),
        search_token(),
    );
    let sentence = execute_search(
        &root,
        "bounded sentence",
        20,
        store.search_candidates("bounded sentence").unwrap(),
        search_token(),
    );

    assert!(
        heading.matches.iter().any(|found| {
            found.kind == WorkspaceSearchMatchKind::Heading && found.start_line == 1
        }),
        "{heading:#?}"
    );
    assert!(
        sentence.matches.iter().any(|found| {
            found.kind == WorkspaceSearchMatchKind::Sentence && found.start_line == 3
        }),
        "{sentence:#?}"
    );
}

#[test]
fn stale_source_is_not_returned_as_current_evidence() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    let original = "fn indexed_needle() {}\n";
    fs::write(workspace.path().join("lib.rs"), original).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("lib.rs", original))
        .unwrap();
    let candidates = store.search_candidates("indexed_needle").unwrap();
    fs::write(workspace.path().join("lib.rs"), "fn changed() {}\n").unwrap();

    let root = workspace.path().canonicalize().unwrap();
    let result = execute_search(&root, "indexed_needle", 50, candidates, search_token());

    assert!(result.matches.is_empty());
    assert!(result.truncated);
}

#[test]
fn result_limit_reports_observed_overflow_before_truncation() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    let source = "needle needle needle\n";
    fs::write(workspace.path().join("notes.log"), source).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("notes.log", source))
        .unwrap();

    let root = workspace.path().canonicalize().unwrap();
    let result = execute_search(
        &root,
        "needle",
        1,
        store.search_candidates("needle").unwrap(),
        search_token(),
    );

    assert_eq!(result.matches.len(), 1, "{result:#?}");
    assert_eq!(result.total_matches, 3);
    assert!(result.truncated);
}

#[test]
fn tyson_scale_candidate_index_finds_a_rare_document_key() {
    let data = tempdir().unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    let mut files = (0..4_749)
        .map(|index| indexed_file(&format!("src/{index:04}.txt"), "ordinary source text"))
        .collect::<Vec<_>>();
    files.push(indexed_file(
        "src/4749.txt",
        "the rare_document_key is indexed here",
    ));
    store.replace_all(&files).unwrap();

    let candidates = store.search_candidates("rare_document_key").unwrap();

    assert_eq!(candidates.indexed_file_count, 4_750);
    assert_eq!(candidates.source_files.len(), 1);
    assert_eq!(candidates.source_files[0].relative_path, "src/4749.txt");
    assert!(!candidates.truncated);
}

#[test]
fn incremental_replacement_and_removal_update_search_atomically() {
    let data = tempdir().unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("lib.rs", "fn alpha_unique() {}"))
        .unwrap();
    assert_eq!(
        store
            .search_candidates("alpha_unique")
            .unwrap()
            .source_files
            .len(),
        1
    );

    store
        .replace_file(&indexed_file("lib.rs", "fn beta_unique() {}"))
        .unwrap();
    assert!(
        store
            .search_candidates("alpha_unique")
            .unwrap()
            .source_files
            .is_empty()
    );
    assert_eq!(
        store
            .search_candidates("beta_unique")
            .unwrap()
            .source_files
            .len(),
        1
    );

    store.remove_file("lib.rs").unwrap();
    assert!(
        store
            .search_candidates("beta_unique")
            .unwrap()
            .source_files
            .is_empty()
    );
}

#[test]
fn semantic_case_recheck_rejects_unicode_folded_fts_candidates() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    let source = "function Äpfel() {}\n";
    fs::write(workspace.path().join("fruit.ts"), source).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("fruit.ts", source))
        .unwrap();
    let root = workspace.path().canonicalize().unwrap();

    let rejected = execute_search(
        &root,
        "äpfel",
        20,
        store.search_candidates("äpfel").unwrap(),
        search_token(),
    );
    assert!(rejected.matches.is_empty(), "{rejected:#?}");

    let accepted = execute_search(
        &root,
        "ÄPFEL",
        20,
        store.search_candidates("ÄPFEL").unwrap(),
        search_token(),
    );
    assert!(accepted.matches.iter().any(|found| {
        found.kind == WorkspaceSearchMatchKind::Symbol && found.match_text == "Äpfel"
    }));
}

#[test]
fn endpoint_outranks_a_path_match_when_limit_is_one() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    fs::create_dir(workspace.path().join("src")).unwrap();
    let route = "const app = router();\napp.get('/users', loadUsers);\n";
    fs::write(workspace.path().join("src/routes.ts"), route).unwrap();
    fs::write(workspace.path().join("src/users.txt"), "plain text\n").unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("src/routes.ts", route),
            indexed_file("src/users.txt", "plain text\n"),
        ])
        .unwrap();
    let root = workspace.path().canonicalize().unwrap();

    let result = execute_search(
        &root,
        "users",
        1,
        store.search_candidates("users").unwrap(),
        search_token(),
    );

    assert_eq!(result.matches.len(), 1, "{result:#?}");
    assert_eq!(result.matches[0].kind, WorkspaceSearchMatchKind::Endpoint);
    assert!(result.truncated);
}

#[test]
fn exactly_fifty_content_matches_are_not_falsely_truncated() {
    for (count, expected_truncated, expected_total) in [(50, false, 50), (51, true, 51)] {
        let workspace = tempdir().unwrap();
        let data = tempdir().unwrap();
        let source = std::iter::repeat_n("needle", count)
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(workspace.path().join("notes.log"), &source).unwrap();
        let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
        store
            .replace_file(&indexed_file("notes.log", &source))
            .unwrap();
        let root = workspace.path().canonicalize().unwrap();

        let result = execute_search(
            &root,
            "needle",
            100,
            store.search_candidates("needle").unwrap(),
            search_token(),
        );

        assert_eq!(result.matches.len(), 50, "count={count}: {result:#?}");
        assert_eq!(result.total_matches, expected_total);
        assert_eq!(result.truncated, expected_truncated);
    }
}

#[test]
fn superseded_search_stops_before_any_source_read() {
    let workspace = tempdir().unwrap();
    let data = tempdir().unwrap();
    let source = "needle\n";
    fs::write(workspace.path().join("notes.txt"), source).unwrap();
    let mut store = GraphStore::open(&data.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file("notes.txt", source))
        .unwrap();
    let candidates = store.search_candidates("needle").unwrap();
    let state = SearchState::new();
    let superseded = state.begin();
    let _latest = state.begin();
    let root = workspace.path().canonicalize().unwrap();

    let (result, stats) = execute_search_with_stats(&root, "needle", 20, candidates, superseded);

    assert!(result.matches.is_empty());
    assert!(result.truncated);
    assert_eq!(stats.read_attempts, 0);
    assert_eq!(stats.bytes_charged, 0);
}

#[test]
fn missing_candidates_cannot_bypass_the_aggregate_read_budget() {
    let workspace = tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let one_mib = 1024 * 1024;
    let source_files = (0..100)
        .map(|index| SourceSearchCandidate {
            relative_path: format!("missing/{index}.txt"),
            content_hash: "stale".into(),
            size_bytes: one_mib as u64,
        })
        .collect();
    let candidates = SearchCandidates {
        indexed_file_count: 100,
        source_files,
        path_files: Vec::new(),
        semantic_nodes: Vec::new(),
        truncated: false,
    };

    let (result, stats) =
        execute_search_with_stats(&root, "needle", 100, candidates, search_token());

    assert!(result.truncated);
    assert_eq!(stats.read_attempts, 64);
    assert_eq!(stats.bytes_charged, MAX_SOURCE_READ_BYTES);
}

#[test]
fn a_file_that_grew_past_the_secure_read_bound_exhausts_the_budget() {
    let workspace = tempdir().unwrap();
    let path = workspace.path().join("grown.txt");
    fs::write(&path, vec![b'x'; 4 * 1024 * 1024 + 1]).unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let candidates = SearchCandidates {
        indexed_file_count: 1,
        source_files: vec![SourceSearchCandidate {
            relative_path: "grown.txt".into(),
            content_hash: blake3::hash(b"x").to_hex().to_string(),
            size_bytes: 1,
        }],
        path_files: Vec::new(),
        semantic_nodes: Vec::new(),
        truncated: false,
    };

    let (result, stats) =
        execute_search_with_stats(&root, "needle", 20, candidates, search_token());

    assert!(result.truncated);
    assert_eq!(stats.read_attempts, 1);
    assert_eq!(stats.bytes_charged, MAX_SOURCE_READ_BYTES);
}
