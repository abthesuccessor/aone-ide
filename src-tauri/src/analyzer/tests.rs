use std::{collections::HashSet, path::Path, time::Instant};

use serde_json::json;
use tree_sitter::Parser;

use crate::domain::{CapabilityLevel, EvidenceKind};

use super::{
    AnalyzedSource, analyze_source,
    extract::{MAX_AST_DEPTH, MAX_FACTS_PER_FILE, analyze_source_with_deadline},
    language_support, stable_id,
    syntax::first_identifier,
    text::{MAX_LABEL_BYTES, compact_text},
};

#[test]
fn extracts_rust_definitions_and_calls_with_provenance() {
    let source = "use crate::db; fn load() { db::query(); }";
    let result = analyze_source("workspace", "src/lib.rs", source, "hash").unwrap();
    assert!(result.nodes.iter().any(|node| node.kind == "function"));
    assert!(result.edges.iter().any(|edge| edge.kind == "calls"));
    assert!(result.edges.iter().any(|edge| edge.kind == "imports"));
    assert!(
        result
            .edges
            .iter()
            .filter(|edge| edge.kind == "calls")
            .all(|edge| edge.evidence == EvidenceKind::Inferred)
    );
}

#[test]
fn supports_every_promised_deep_language() {
    let cases = [
        "a.rs", "a.ts", "a.tsx", "a.js", "a.py", "a.go", "A.java", "A.cs", "a.cpp", "A.kt",
    ];
    for path in cases {
        let expected = if path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".js") {
            CapabilityLevel::Semantic
        } else {
            CapabilityLevel::SyntaxOnly
        };
        assert_eq!(
            language_support(Path::new(path)).capability,
            expected,
            "{path}"
        );
    }
}

#[test]
fn every_promised_grammar_parses_a_real_definition() {
    let cases = [
        ("src/a.rs", "fn run() {}"),
        ("src/a.ts", "function run(): void {}"),
        ("src/a.js", "function run() {}"),
        ("src/a.py", "def run():\n    pass\n"),
        ("src/a.go", "package a\nfunc run() {}"),
        ("src/A.java", "class A { void run() {} }"),
        ("src/A.cs", "class A { void Run() {} }"),
        ("src/a.cpp", "void run() {}"),
        ("src/A.kt", "fun run() {}"),
    ];
    for (path, source) in cases {
        let result = analyze_source("workspace", path, source, "hash").unwrap();
        assert!(
            result
                .nodes
                .iter()
                .any(|node| matches!(node.kind.as_str(), "function" | "method")),
            "no definition extracted for {path}: {:#?}",
            result.nodes
        );
    }
}

#[test]
fn extracts_typescript_framework_semantics() {
    let source = r#"
      class UserRepository { find() {} }
      router.get('/users', handler);
      fetch('/api/users');
      prisma.user.findMany();
    "#;
    let result = analyze_source("workspace", "src/users.ts", source, "hash").unwrap();
    assert!(result.nodes.iter().any(|node| node.kind == "repository"));
    assert!(result.nodes.iter().any(|node| node.kind == "endpoint"));
    assert!(result.nodes.iter().any(|node| node.kind == "api"));
    assert!(result.nodes.iter().any(|node| node.kind == "database"));
    assert!(result.edges.iter().any(|edge| edge.kind == "handles"));
    assert!(result.edges.iter().any(|edge| edge.kind == "requests"));
    assert!(result.edges.iter().any(|edge| edge.kind == "queries"));
}

#[test]
fn extracts_static_javascript_event_semantics_with_literal_provenance() {
    let source = r#"function wire(socket: Socket) {
  window.addEventListener('resize', handler);
  socket.on("message", handler);
  emitter.once(`ready`, handler);
  emitter.addListener('close', handler);
  listen<Payload>('sync', handler);
  appWindow.listen('tauri://drag-drop', handler);
  emit('saved', payload);
  emitter.emit('message', payload);
  bus.dispatch('refresh', payload);
  broker.publish(`job.created`, payload);
  window.dispatchEvent(new CustomEvent('app:ready'));
}"#;
    let result = analyze_source("workspace", "src/events.ts", source, "hash").unwrap();
    let events = result
        .nodes
        .iter()
        .filter(|node| node.kind == "event")
        .collect::<Vec<_>>();

    assert_eq!(events.len(), 11, "unexpected events: {events:#?}");
    assert!(events.iter().all(|node| {
        node.evidence == EvidenceKind::Inferred
            && node.metadata.get("staticEventName") == Some(&json!(true))
            && node
                .metadata
                .get("confidence")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|confidence| (0.0..=1.0).contains(&confidence))
    }));
    assert_eq!(
        events
            .iter()
            .find(|node| {
                node.label == "message" && node.metadata.get("eventOperation") == Some(&json!("on"))
            })
            .and_then(|node| node.source.as_ref())
            .map(|source| (source.start_line, source.start_column, source.end_column)),
        Some((3, 13, 22))
    );
    let dispatched = events
        .iter()
        .find(|node| node.label == "app:ready")
        .unwrap();
    assert_eq!(
        dispatched.metadata.get("eventOperation"),
        Some(&json!("dispatchEvent"))
    );
    assert_eq!(dispatched.metadata.get("eventApi"), Some(&json!("DOM")));
    assert_eq!(
        dispatched.metadata.get("eventDirection"),
        Some(&json!("publication"))
    );
    assert_eq!(
        dispatched.source.as_ref().map(|source| source.start_line),
        Some(12)
    );
    assert!(events.iter().all(|node| {
        node.source.as_ref().is_some_and(|source| {
            source.start_line == source.end_line
                && source.end_column - source.start_column == node.label.len() + 2
        })
    }));
    assert_eq!(
        result
            .edges
            .iter()
            .filter(|edge| edge.kind == "listensTo")
            .count(),
        6
    );
    assert_eq!(
        result
            .edges
            .iter()
            .filter(|edge| edge.kind == "emits")
            .count(),
        5
    );
}

#[test]
fn ast_and_event_columns_are_monaco_utf16_units() {
    let source = "const 東京 = '😀'; function load() {}\n\
                  const 値 = '😀'; socket.on('受信😀', handler);";
    let result = analyze_source("workspace", "src/unicode.ts", source, "hash").unwrap();
    let definition = result
        .nodes
        .iter()
        .find(|node| node.kind == "function" && node.label == "load")
        .and_then(|node| node.source.as_ref())
        .unwrap();
    assert_eq!(
        (
            definition.start_line,
            definition.start_column,
            definition.end_column
        ),
        (1, 18, 36)
    );
    let event = result
        .nodes
        .iter()
        .find(|node| node.kind == "event" && node.label == "受信😀")
        .and_then(|node| node.source.as_ref())
        .unwrap();
    assert_eq!(
        (event.start_line, event.start_column, event.end_column),
        (2, 27, 33)
    );
}

#[test]
fn dynamic_event_names_and_unrelated_listen_calls_are_not_claimed_as_events() {
    let source = r#"
      socket.on(eventName, handler);
      socket.once(`${scope}:ready`, handler);
      socket.addListener(handler, 'late-string');
      emit(eventName, payload);
      bus.publish(topic, payload);
      window.dispatchEvent(new CustomEvent(eventName));
      server.listen('/tmp/app.sock');
      socket.on('user\nready', handler);
      socket.on('missing-handler');
    "#;
    let result = analyze_source("workspace", "src/dynamic.ts", source, "hash").unwrap();

    assert!(!result.nodes.iter().any(|node| node.kind == "event"));
    assert!(
        !result
            .edges
            .iter()
            .any(|edge| matches!(edge.kind.as_str(), "listensTo" | "emits"))
    );
}

#[test]
fn event_ids_survive_unrelated_body_edits_and_duplicate_events_remain_unique() {
    let before = analyze_source(
        "workspace",
        "src/socket.ts",
        "function wire() { prep(); socket.on('message', handler); socket.on('message', audit); }",
        "before",
    )
    .unwrap();
    let after = analyze_source(
        "workspace",
        "src/socket.ts",
        "function wire() { changed('body'); prep(); socket.on('message', handler); socket.on('message', audit); }",
        "after",
    )
    .unwrap();
    let event_ids = |analysis: &AnalyzedSource| {
        analysis
            .nodes
            .iter()
            .filter(|node| node.kind == "event")
            .map(|node| node.id.clone())
            .collect::<Vec<_>>()
    };
    let before_ids = event_ids(&before);

    assert_eq!(before_ids.len(), 2);
    assert_eq!(before_ids.iter().collect::<HashSet<_>>().len(), 2);
    assert_eq!(before_ids, event_ids(&after));
}

#[test]
fn identifiers_are_deterministic_and_namespace_safe() {
    assert_eq!(
        stable_id("file", &["a", "bc"]),
        stable_id("file", &["a", "bc"])
    );
    assert_ne!(
        stable_id("file", &["a", "bc"]),
        stable_id("file", &["ab", "c"])
    );
}

#[test]
fn symbol_identity_survives_body_edits_and_duplicate_calls_remain_unique() {
    let before = analyze_source(
        "workspace",
        "src/service.ts",
        "function load(id: string) { one(); one(); }",
        "before",
    )
    .unwrap();
    let after = analyze_source(
        "workspace",
        "src/service.ts",
        "function load(id: string) { two(); }",
        "after",
    )
    .unwrap();
    let before_function = before
        .nodes
        .iter()
        .find(|node| node.kind == "function")
        .unwrap();
    let after_function = after
        .nodes
        .iter()
        .find(|node| node.kind == "function")
        .unwrap();
    assert_eq!(before_function.id, after_function.id);

    let call_ids = before
        .nodes
        .iter()
        .filter(|node| node.kind == "callTarget")
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(call_ids.len(), 2);
}

#[test]
fn repeated_file_scoped_typescript_definitions_have_unique_stable_ids() {
    let before = (0..9)
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
    let after = before.replace("useCase(4)", "useCaseChanged(4)");

    let wrapper_ids = |source: &str, hash: &str| {
        analyze_source("workspace", "src/repeated.test.tsx", source, hash)
            .unwrap()
            .nodes
            .into_iter()
            .filter(|node| node.kind == "function" && node.label == "wrapper")
            .map(|node| node.id)
            .collect::<Vec<_>>()
    };
    let before_ids = wrapper_ids(&before, "before");
    let after_ids = wrapper_ids(&after, "after");

    assert_eq!(before_ids.len(), 9);
    assert_eq!(before_ids.iter().collect::<HashSet<_>>().len(), 9);
    assert_eq!(before_ids, after_ids);
}

#[test]
fn repeated_rust_associated_types_in_distinct_implementations_are_unique() {
    let source = r#"
        impl FromRequestParts<AppState> for AuthUser {
            type Rejection = AppError;
        }

        impl OptionalFromRequestParts<AppState> for AuthUser {
            type Rejection = AppError;
        }
    "#;
    let result = analyze_source("workspace", "src/extractor.rs", source, "hash").unwrap();
    let rejection_ids = result
        .nodes
        .iter()
        .filter(|node| node.kind == "type" && node.label == "Rejection")
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(rejection_ids.len(), 2);
    assert_eq!(
        rejection_ids.iter().copied().collect::<HashSet<_>>().len(),
        2
    );
    assert_eq!(
        result
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        result.nodes.len()
    );
    assert_eq!(
        result
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        result.edges.len()
    );
}

#[test]
fn deeply_nested_ast_is_bounded_and_truthfully_marked_truncated() {
    let nesting = MAX_AST_DEPTH + 64;
    let source = format!("{}value{};", "(".repeat(nesting), ")".repeat(nesting));
    let result = analyze_source("workspace", "src/deep.ts", &source, "hash").unwrap();
    let file_node = result
        .nodes
        .iter()
        .find(|node| node.kind == "file")
        .unwrap();

    assert!(result.parse_errors);
    assert_eq!(
        file_node.metadata.get("analysisTruncated"),
        Some(&json!(true))
    );
    assert!(
        file_node
            .metadata
            .get("analysisTruncationReasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| reason.as_str() == Some("AST depth limit reached")))
    );

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .unwrap();
    let tree = parser.parse(&source, None).unwrap();
    let mut identifier_search_truncated = false;
    assert!(
        first_identifier(tree.root_node(), &source, &mut identifier_search_truncated).is_none()
    );
    assert!(identifier_search_truncated);
}

#[test]
fn per_file_fact_limit_is_truthfully_marked_truncated() {
    let calls = "work();".repeat(MAX_FACTS_PER_FILE + 16);
    let source = format!("function run() {{ {calls} }}");
    let result = analyze_source("workspace", "src/many.ts", &source, "hash").unwrap();
    let file_node = result
        .nodes
        .iter()
        .find(|node| node.kind == "file")
        .unwrap();

    assert_eq!(result.fact_count, MAX_FACTS_PER_FILE);
    assert!(result.parse_errors);
    assert_eq!(
        file_node.metadata.get("analysisTruncated"),
        Some(&json!(true))
    );
    assert!(
        file_node
            .metadata
            .get("analysisTruncationReasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| reason.as_str() == Some("per-file fact limit reached")))
    );
}

#[test]
fn expired_analysis_deadline_stops_before_ast_extraction() {
    let error = analyze_source_with_deadline(
        "workspace",
        "src/deadline.ts",
        "function run() { work(); }",
        "hash",
        Some(Instant::now()),
    )
    .unwrap_err();
    assert!(error.to_string().contains("wall-clock budget"));
}

#[test]
fn compact_text_bounds_large_utf8_subtrees_without_collecting_them() {
    let source = format!("const value = '{}';", "界".repeat(20_000));
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .unwrap();
    let tree = parser.parse(&source, None).unwrap();

    let compact = compact_text(tree.root_node(), &source);
    assert!(compact.ends_with('…'));
    assert!(compact.len() <= MAX_LABEL_BYTES + '…'.len_utf8());
    assert!(compact.is_char_boundary(compact.len()));
}

#[test]
fn large_call_subtree_ids_are_deterministic_without_subtree_hashing() {
    let source = format!("function run() {{ work('{}'); }}", "x".repeat(100_000));
    let first = analyze_source("workspace", "src/large.ts", &source, "content-hash").unwrap();
    let second = analyze_source("workspace", "src/large.ts", &source, "content-hash").unwrap();
    let call_ids = |analysis: &AnalyzedSource| {
        analysis
            .nodes
            .iter()
            .filter(|node| node.kind == "callTarget")
            .map(|node| node.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(call_ids(&first), call_ids(&second));
}
