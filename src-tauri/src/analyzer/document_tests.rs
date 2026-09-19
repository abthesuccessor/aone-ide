use serde_json::json;

use crate::domain::{EvidenceKind, GraphEdge};

use super::{
    analyze_source, document::MAX_DOCUMENT_FACTS_PER_FILE,
    document_structure::MAX_DOCUMENT_LINES_PER_FILE, types::AnalyzedSource,
};

fn node_id(analysis: &AnalyzedSource, label: &str) -> String {
    analysis
        .nodes
        .iter()
        .find(|node| node.label == label)
        .unwrap_or_else(|| panic!("missing node `{label}` in {:#?}", analysis.nodes))
        .id
        .clone()
}

fn has_edge(edges: &[GraphEdge], source: &str, target: &str, kind: &str) -> bool {
    edges
        .iter()
        .any(|edge| edge.source == source && edge.target == target && edge.kind == kind)
}

#[test]
fn markdown_extracts_heading_hierarchy_sentences_and_reading_order() {
    let source = "# Intro\n\nOne sentence. Second sentence!\n\n## Detail\nThird line\ncontinues?\n";
    let analysis = analyze_source("workspace", "guide.md", source, "hash").unwrap();
    let file = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "file")
        .unwrap();
    let intro = node_id(&analysis, "Intro");
    let one = node_id(&analysis, "One sentence.");
    let two = node_id(&analysis, "Second sentence!");
    let detail = node_id(&analysis, "Detail");
    let three = node_id(&analysis, "Third line continues?");

    assert_eq!(analysis.fact_count, 5);
    assert!(has_edge(&analysis.edges, &file.id, &intro, "contains"));
    assert!(has_edge(&analysis.edges, &intro, &one, "contains"));
    assert!(has_edge(&analysis.edges, &intro, &two, "contains"));
    assert!(has_edge(&analysis.edges, &intro, &detail, "contains"));
    assert!(has_edge(&analysis.edges, &detail, &three, "contains"));
    assert!(has_edge(&analysis.edges, &intro, &one, "precedes"));
    assert!(has_edge(&analysis.edges, &one, &two, "precedes"));
    assert!(has_edge(&analysis.edges, &two, &detail, "precedes"));
    assert!(has_edge(&analysis.edges, &detail, &three, "precedes"));
    assert!(
        analysis
            .nodes
            .iter()
            .filter(|node| node.kind == "heading" || node.kind == "sentence")
            .all(|node| node.evidence == EvidenceKind::Declared)
    );
    assert!(
        analysis
            .edges
            .iter()
            .all(|edge| { edge.evidence == EvidenceKind::Declared && edge.confidence.is_none() })
    );
}

#[test]
fn markdown_and_asciidoc_fenced_code_is_not_document_prose() {
    let markdown = "# Visible\n\nBefore.\n\n```rust\n# Fake\nhidden sentence.\n```\n\nAfter.\n";
    let markdown_analysis = analyze_source("workspace", "guide.mdx", markdown, "hash").unwrap();
    let labels = markdown_analysis
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Visible"));
    assert!(labels.contains(&"Before."));
    assert!(labels.contains(&"After."));
    assert!(!labels.iter().any(|label| label.contains("Fake")));
    assert!(!labels.iter().any(|label| label.contains("hidden")));

    let plain = "Before.\n```\nhidden sentence.\n```\nAfter.";
    let plain_analysis = analyze_source("workspace", "notes.txt", plain, "hash").unwrap();
    assert!(
        !plain_analysis
            .nodes
            .iter()
            .any(|node| node.label.contains("hidden"))
    );

    let asciidoc = "= Visible\n\n[source,rust]\n----\n= Fake\nhidden sentence.\n----\n\nAfter.\n";
    let asciidoc_analysis = analyze_source("workspace", "guide.adoc", asciidoc, "hash").unwrap();
    assert!(
        !asciidoc_analysis
            .nodes
            .iter()
            .any(|node| node.label.contains("Fake") || node.label.contains("hidden"))
    );
}

#[test]
fn document_ranges_use_exact_exclusive_utf16_columns() {
    let source = "# 😀 Plan ###\r\n\r\n😀 Starts here\r\nand ends.\r\n";
    let analysis = analyze_source("workspace", "unicode.md", source, "hash").unwrap();
    let heading = analysis
        .nodes
        .iter()
        .find(|node| node.label == "😀 Plan")
        .and_then(|node| node.source.as_ref())
        .unwrap();
    let sentence = analysis
        .nodes
        .iter()
        .find(|node| node.label == "😀 Starts here and ends.")
        .and_then(|node| node.source.as_ref())
        .unwrap();

    assert_eq!(
        (
            heading.start_line,
            heading.start_column,
            heading.end_line,
            heading.end_column
        ),
        (1, 3, 1, 10)
    );
    assert_eq!(
        (
            sentence.start_line,
            sentence.start_column,
            sentence.end_line,
            sentence.end_column
        ),
        (3, 1, 4, 10)
    );
}

#[test]
fn supported_document_extensions_route_to_deterministic_structure() {
    let cases = [
        ("notes.txt", "Plain text works.", "Text", "plainText", 0),
        (
            "guide.rst",
            "Guide\n=====\nRST works.\n",
            "reStructuredText",
            "reStructuredText",
            1,
        ),
        (
            "guide.asciidoc",
            "= Guide\n\nAsciiDoc works.\n",
            "AsciiDoc",
            "asciiDoc",
            1,
        ),
    ];
    for (path, source, language, format, heading_count) in cases {
        let analysis = analyze_source("workspace", path, source, "hash").unwrap();
        let file = analysis
            .nodes
            .iter()
            .find(|node| node.kind == "file")
            .unwrap();
        assert_eq!(file.language.as_deref(), Some(language), "{path}");
        assert_eq!(file.metadata.get("parser"), Some(&json!("aone-document")));
        assert_eq!(file.metadata.get("documentFormat"), Some(&json!(format)));
        assert_eq!(
            analysis
                .nodes
                .iter()
                .filter(|node| node.kind == "heading")
                .count(),
            heading_count,
            "{path}"
        );
        assert!(analysis.nodes.iter().any(|node| node.kind == "sentence"));
    }
}

#[test]
fn document_ids_are_stable_for_unchanged_segments_and_duplicates_are_unique() {
    let first = analyze_source(
        "workspace",
        "guide.md",
        "# Guide\n\nSame. Same. Target sentence.\n",
        "hash-one",
    )
    .unwrap();
    let second = analyze_source(
        "workspace",
        "guide.md",
        "# Guide\n\nSame. Same. Added sentence. Target sentence.\n",
        "hash-two",
    )
    .unwrap();
    let duplicate_ids = first
        .nodes
        .iter()
        .filter(|node| node.label == "Same.")
        .map(|node| &node.id)
        .collect::<Vec<_>>();

    assert_eq!(duplicate_ids.len(), 2);
    assert_ne!(duplicate_ids[0], duplicate_ids[1]);
    assert_eq!(node_id(&first, "Guide"), node_id(&second, "Guide"));
    assert_eq!(
        node_id(&first, "Target sentence."),
        node_id(&second, "Target sentence.")
    );
}

#[test]
fn document_fact_limit_sets_truthful_truncation_metadata() {
    let source = "Bounded sentence. ".repeat(MAX_DOCUMENT_FACTS_PER_FILE + 1);
    let analysis = analyze_source("workspace", "large.txt", &source, "hash").unwrap();
    let file = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "file")
        .unwrap();

    assert_eq!(analysis.fact_count, MAX_DOCUMENT_FACTS_PER_FILE);
    assert!(analysis.parse_errors);
    assert_eq!(file.metadata.get("analysisTruncated"), Some(&json!(true)));
    assert_eq!(
        file.metadata.get("factsExtracted"),
        Some(&json!(MAX_DOCUMENT_FACTS_PER_FILE))
    );
    assert!(
        file.metadata
            .get("analysisTruncationReasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| reason == "document fact limit reached"))
    );
}

#[test]
fn document_line_limit_sets_truthful_truncation_metadata() {
    let source = "\n".repeat(MAX_DOCUMENT_LINES_PER_FILE + 1);
    let analysis = analyze_source("workspace", "many-lines.txt", &source, "hash").unwrap();
    let file = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "file")
        .unwrap();

    assert!(analysis.parse_errors);
    assert_eq!(
        file.metadata.get("documentLinesVisited"),
        Some(&json!(MAX_DOCUMENT_LINES_PER_FILE))
    );
    assert!(
        file.metadata
            .get("analysisTruncationReasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| reason == "document line limit reached"))
    );
}

#[test]
fn non_document_text_formats_keep_the_file_only_behavior() {
    for (path, source) in [
        ("config.toml", "name = 'aone'"),
        ("data.json", "{\"value\": 1}"),
        ("query.sql", "select 1;"),
    ] {
        let analysis = analyze_source("workspace", path, source, "hash").unwrap();
        assert_eq!(analysis.fact_count, 0, "{path}");
        assert_eq!(analysis.nodes.len(), 1, "{path}");
        assert!(analysis.edges.is_empty(), "{path}");
        assert_eq!(analysis.nodes[0].kind, "file", "{path}");
    }
}
