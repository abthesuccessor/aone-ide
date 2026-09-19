use serde_json::json;
use tree_sitter::Node;

use crate::domain::{EvidenceKind, GraphNode, SourceLocation};

use super::{api::HttpMetadata, api_javascript::typescript_http_call, text::compact_text};

const MAX_IDENTIFIER_NODE_VISITS: usize = 4_096;
const MAX_IDENTIFIER_DEPTH: usize = 128;
const MAX_EVENT_NAME_BYTES: usize = 240;

pub(super) fn location(relative_path: &str, node: Node<'_>, source: &str) -> SourceLocation {
    let start = node.start_position();
    let end = node.end_position();
    SourceLocation {
        relative_path: relative_path.into(),
        start_line: start.row + 1,
        start_column: utf16_column(source, node.start_byte(), start.column),
        end_line: end.row + 1,
        end_column: utf16_column(source, node.end_byte(), end.column),
    }
}

fn utf16_column(source: &str, byte_offset: usize, byte_column: usize) -> usize {
    let line_start = byte_offset.saturating_sub(byte_column);
    source
        .get(line_start..byte_offset)
        .map_or(byte_column + 1, |prefix| prefix.encode_utf16().count() + 1)
}

pub(super) fn definition_kind(
    node: Node<'_>,
    source: &str,
    identifier_search_truncated: &mut bool,
) -> Option<&'static str> {
    match node.kind() {
        "function_item"
        | "function_declaration"
        | "function_definition"
        | "function_definition_item"
        | "function_expression"
        | "function_definition_header"
        | "local_function_statement" => Some("function"),
        "method_definition"
        | "method_declaration"
        | "method_definition_item"
        | "constructor_declaration"
        | "secondary_constructor" => Some("method"),
        "class_declaration" | "class_definition" | "class_specifier" => Some("class"),
        "interface_declaration" => Some("interface"),
        "trait_item" => Some("trait"),
        "struct_item" | "struct_declaration" | "struct_specifier" => Some("struct"),
        "enum_item" | "enum_declaration" | "enum_specifier" => Some("enum"),
        "impl_item" => Some("implementation"),
        "mod_item" | "module_declaration" | "namespace_definition" => Some("module"),
        "object_declaration" | "companion_object" => Some("object"),
        "type_alias_declaration" | "type_item" => Some("type"),
        "variable_declarator"
            if node.child_by_field_name("value").is_some_and(|value| {
                matches!(value.kind(), "arrow_function" | "function_expression")
            }) && node_name(node, source, identifier_search_truncated).is_some() =>
        {
            Some("function")
        }
        _ => None,
    }
}

pub(super) fn classify_definition(
    kind: &'static str,
    label: &str,
    language: &str,
) -> (&'static str, EvidenceKind, Option<&'static str>) {
    if !matches!(language, "TypeScript" | "JavaScript") || kind != "class" {
        return (kind, EvidenceKind::Declared, None);
    }
    let lower = label.to_ascii_lowercase();
    for (suffix, role) in [
        ("repository", "repository"),
        ("service", "service"),
        ("controller", "controller"),
        ("model", "model"),
    ] {
        if lower.ends_with(suffix) {
            return (role, EvidenceKind::Inferred, Some(role));
        }
    }
    (kind, EvidenceKind::Declared, None)
}

pub(super) struct SemanticCall {
    pub(super) node_kind: &'static str,
    pub(super) edge_kind: &'static str,
    pub(super) label: String,
    pub(super) confidence: f64,
    pub(super) event: Option<EventSemantic>,
    pub(super) http: Option<HttpMetadata>,
}

pub(super) struct EventSemantic {
    pub(super) name: String,
    pub(super) direction: &'static str,
    pub(super) operation: String,
    pub(super) api: String,
    pub(super) source: SourceLocation,
}

impl EventSemantic {
    pub(super) fn decorate(&self, node: &mut GraphNode, confidence: f64) {
        node.source = Some(self.source.clone());
        node.metadata.insert("confidence".into(), json!(confidence));
        node.metadata
            .insert("eventApi".into(), json!(self.api.as_str()));
        node.metadata
            .insert("eventDirection".into(), json!(self.direction));
        node.metadata
            .insert("eventOperation".into(), json!(self.operation.as_str()));
        node.metadata.insert("staticEventName".into(), json!(true));
    }
}

pub(super) fn semantic_call(
    node: Node<'_>,
    source: &str,
    language: &str,
    relative_path: &str,
) -> Option<SemanticCall> {
    if !matches!(language, "TypeScript" | "JavaScript") {
        return None;
    }
    let callee = call_name(node, source)?;
    let argument = first_static_string_argument(node, source).map(|(value, _)| value);
    let lower = callee.to_ascii_lowercase();

    if let Some(event) = event_semantic(node, source, relative_path, &callee) {
        let (edge_kind, confidence) = match event.direction {
            "subscription" => (
                "listensTo",
                subscription_confidence(&event.operation, &event.api),
            ),
            _ => ("emits", publication_confidence(&event.operation)),
        };
        let label = event.name.clone();
        return Some(SemanticCall {
            node_kind: "event",
            edge_kind,
            label,
            confidence,
            event: Some(event),
            http: None,
        });
    }

    let is_redis = lower == "redis"
        || lower.starts_with("redis.")
        || lower.contains(".redis.")
        || lower.starts_with("redisclient.");
    let is_database = is_redis
        || lower == "db.query"
        || lower == "db.execute"
        || lower.ends_with(".db.query")
        || lower.ends_with(".db.execute")
        || lower.starts_with("prisma.")
        || lower.contains(".prisma.")
        || lower.starts_with("sequelize.")
        || lower.contains(".sequelize.")
        || lower.starts_with("knex.")
        || lower.contains(".knex.");
    if is_database {
        return Some(SemanticCall {
            node_kind: "database",
            edge_kind: if is_redis { "accesses" } else { "queries" },
            label: argument.map_or_else(|| callee.clone(), |target| format!("{callee} {target}")),
            confidence: 0.88,
            event: None,
            http: None,
        });
    }

    if let Some(http) = typescript_http_call(node, source, relative_path, &callee) {
        return Some(SemanticCall {
            node_kind: http.node_kind,
            edge_kind: http.edge_kind,
            label: http.label,
            confidence: http.confidence,
            event: None,
            http: Some(http.metadata),
        });
    }
    None
}

fn event_semantic(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    callee: &str,
) -> Option<EventSemantic> {
    let operation = terminal_operation(callee);
    let lower = operation.to_ascii_lowercase();
    let is_member = callee.contains('.') || callee.contains("?.");
    let api = event_api(callee, &lower, is_member)?;
    let direction = if matches!(
        lower.as_str(),
        "addeventlistener" | "on" | "once" | "addlistener" | "listen"
    ) {
        "subscription"
    } else {
        "publication"
    };
    if direction == "subscription"
        && node
            .child_by_field_name("arguments")
            .is_none_or(|arguments| arguments.named_child_count() < 2)
    {
        return None;
    }
    let (name, literal) = event_name(node, source, &operation)?;
    Some(EventSemantic {
        name,
        direction,
        operation,
        api,
        source: location(relative_path, literal, source),
    })
}

fn event_api(callee: &str, operation: &str, is_member: bool) -> Option<String> {
    let receiver = callee
        .rsplit_once('.')
        .map(|(receiver, _)| receiver.trim_end_matches('?'));
    let receiver_lower = receiver.unwrap_or_default().to_ascii_lowercase();
    let family = match operation {
        "addeventlistener" | "dispatchevent" => "DOM",
        "on" | "once" | "addlistener" if is_member => "EventEmitter/Socket",
        "listen"
            if !is_member
                || receiver_lower.contains("tauri")
                || receiver_lower.contains("window")
                || receiver_lower.contains("webview") =>
        {
            "Tauri"
        }
        "emit" => "EventEmitter/Tauri/Socket",
        "dispatch" => "Dispatcher",
        "publish" => "Publisher",
        _ => return None,
    };
    Some(family.into())
}

fn event_name<'tree>(
    node: Node<'tree>,
    source: &str,
    operation: &str,
) -> Option<(String, Node<'tree>)> {
    let first = first_argument(node)?;
    if operation.eq_ignore_ascii_case("dispatchEvent") && first.kind() == "new_expression" {
        let constructor = first.child_by_field_name("constructor")?;
        let name = compact_text(constructor, source);
        if !matches!(name.as_str(), "Event" | "CustomEvent") {
            return None;
        }
        return first_static_string_argument(first, source);
    }
    static_string_literal(first, source)
}

fn first_argument(node: Node<'_>) -> Option<Node<'_>> {
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    arguments.named_children(&mut cursor).next()
}

fn first_static_string_argument<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, Node<'tree>)> {
    static_string_literal(first_argument(node)?, source)
}

fn static_string_literal<'tree>(node: Node<'tree>, source: &str) -> Option<(String, Node<'tree>)> {
    if node.kind() == "template_string" {
        let mut cursor = node.walk();
        if node
            .named_children(&mut cursor)
            .any(|child| child.kind() == "template_substitution")
        {
            return None;
        }
    } else if node.kind() != "string" {
        return None;
    }
    let raw = source.get(node.byte_range())?;
    if raw.len() < 2 || raw.len() > MAX_EVENT_NAME_BYTES + 2 {
        return None;
    }
    let delimiter = *raw.as_bytes().first()?;
    if !matches!(delimiter, b'\'' | b'"' | b'`') || raw.as_bytes().last() != Some(&delimiter) {
        return None;
    }
    let value = &raw[1..raw.len() - 1];
    if value.trim().is_empty() || value.contains('\\') || value.chars().any(char::is_control) {
        return None;
    }
    Some((value.to_owned(), node))
}

fn terminal_operation(callee: &str) -> String {
    callee
        .rsplit('.')
        .next()
        .unwrap_or(callee)
        .trim_start_matches('?')
        .split('<')
        .next()
        .unwrap_or(callee)
        .to_owned()
}

fn subscription_confidence(operation: &str, api: &str) -> f64 {
    match operation.to_ascii_lowercase().as_str() {
        "addeventlistener" => 0.99,
        "listen" if api == "Tauri" => 0.94,
        "addlistener" => 0.93,
        _ => 0.88,
    }
}

fn publication_confidence(operation: &str) -> f64 {
    match operation.to_ascii_lowercase().as_str() {
        "dispatchevent" => 0.99,
        "emit" => 0.93,
        _ => 0.86,
    }
}

pub(super) fn is_import(kind: &str) -> bool {
    matches!(
        kind,
        "import_statement"
            | "import_declaration"
            | "use_declaration"
            | "using_directive"
            | "preproc_include"
            | "package_clause"
    )
}

pub(super) fn is_call(kind: &str) -> bool {
    matches!(
        kind,
        "call_expression" | "call" | "method_invocation" | "invocation_expression"
    )
}

pub(super) fn node_name(
    node: Node<'_>,
    source: &str,
    identifier_search_truncated: &mut bool,
) -> Option<String> {
    for field in ["name", "declarator", "type"] {
        if let Some(child) = node.child_by_field_name(field)
            && let Some(value) = first_identifier(child, source, identifier_search_truncated)
        {
            return Some(value);
        }
    }
    first_identifier(node, source, identifier_search_truncated)
}

pub(super) fn first_identifier(
    node: Node<'_>,
    source: &str,
    identifier_search_truncated: &mut bool,
) -> Option<String> {
    let mut visited = 0_usize;
    let mut stack = vec![(node, 0_usize)];

    while let Some((current, depth)) = stack.pop() {
        if visited >= MAX_IDENTIFIER_NODE_VISITS {
            *identifier_search_truncated = true;
            break;
        }
        if depth > MAX_IDENTIFIER_DEPTH {
            *identifier_search_truncated = true;
            continue;
        }
        visited += 1;

        if matches!(
            current.kind(),
            "identifier" | "type_identifier" | "field_identifier" | "property_identifier"
        ) {
            let value = compact_text(current, source);
            if !value.is_empty() {
                return Some(value);
            }
        }

        let child_count = current.named_child_count();
        if depth >= MAX_IDENTIFIER_DEPTH {
            if child_count > 0 {
                *identifier_search_truncated = true;
            }
            continue;
        }
        let remaining_capacity = MAX_IDENTIFIER_NODE_VISITS
            .saturating_sub(visited)
            .saturating_sub(stack.len());
        let scheduled_children = child_count.min(remaining_capacity);
        if scheduled_children < child_count {
            *identifier_search_truncated = true;
        }
        for index in (0..scheduled_children).rev() {
            let Ok(child_index) = u32::try_from(index) else {
                *identifier_search_truncated = true;
                break;
            };
            if let Some(child) = current.named_child(child_index) {
                stack.push((child, depth + 1));
            }
        }
    }

    None
}

pub(super) fn call_name(node: Node<'_>, source: &str) -> Option<String> {
    for field in ["function", "name"] {
        if let Some(child) = node.child_by_field_name(field) {
            let value = compact_text(child, source);
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

pub(super) fn definition_signature(node: Node<'_>, source: &str) -> String {
    let mut parts = Vec::new();
    for field in [
        "type_parameters",
        "parameters",
        "return_type",
        "superclasses",
    ] {
        if let Some(child) = node.child_by_field_name(field) {
            let text = compact_text(child, source);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    parts.join(" ")
}
