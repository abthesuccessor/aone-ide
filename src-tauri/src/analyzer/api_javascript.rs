use tree_sitter::Node;

use super::{
    api::{
        HTTP_METHODS, HttpCall, HttpHandler, HttpMetadata, bounded_api_label, is_relative_route,
        sanitized_http_target,
    },
    syntax::location,
    text::{MAX_LABEL_BYTES, compact_text},
};

const MAX_STATIC_ROUTE_BYTES: usize = 1_024;

pub(super) fn typescript_http_call(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    callee: &str,
) -> Option<HttpCall> {
    let lower = callee.to_ascii_lowercase();
    let operation = lower.rsplit('.').next().unwrap_or(lower.as_str());
    let direct_method = HTTP_METHODS
        .iter()
        .find(|candidate| operation == **candidate)
        .map(|value| value.to_ascii_uppercase());
    let is_fetch = lower == "fetch" || lower.ends_with(".fetch");
    let is_request = lower.ends_with(".request");
    let is_axios = lower == "axios" || lower.starts_with("axios.");
    let arguments = call_arguments(node);
    let mut target_node = arguments.first().copied();
    let mut target = target_node
        .and_then(|argument| static_javascript_string(argument, source))
        .and_then(|value| sanitized_http_target(&value));

    if let Some(method) = direct_method {
        return direct_method_call(
            &lower,
            method,
            &arguments,
            target_node,
            target,
            source,
            relative_path,
        );
    }

    if !(is_fetch || is_request || is_axios) {
        return None;
    }
    let framework = if is_axios {
        Some("Axios")
    } else if is_fetch {
        Some("Fetch")
    } else {
        None
    };
    let method = if is_fetch {
        fetch_method(&arguments, source)
    } else if is_request || lower == "axios" {
        request_configuration(&arguments, source, &mut target_node, &mut target)
    } else {
        None
    };
    Some(HttpCall {
        node_kind: "api",
        edge_kind: "requests",
        label: match (method.as_deref(), target.as_deref()) {
            (Some(method), Some(target)) => bounded_api_label(method, target),
            (_, Some(target)) => bounded_api_label(callee, target),
            _ => callee.to_owned(),
        },
        confidence: 0.9,
        metadata: HttpMetadata {
            role: "consumer",
            method,
            path: target,
            framework,
            source: target_node.map(|target| location(relative_path, target, source)),
            handler: None,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn direct_method_call(
    lower: &str,
    method: String,
    arguments: &[Node<'_>],
    target_node: Option<Node<'_>>,
    target: Option<String>,
    source: &str,
    relative_path: &str,
) -> Option<HttpCall> {
    let operation = lower.rsplit('.').next().unwrap_or(lower);
    let receiver = lower
        .strip_suffix(operation)
        .unwrap_or_default()
        .trim_end_matches('.')
        .trim_end_matches('?');
    let terminal_receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    let handler = route_handler(arguments, source, relative_path);
    let producer_receiver = matches!(terminal_receiver, "app" | "router" | "server" | "fastify")
        || terminal_receiver.ends_with("router");
    let producer = producer_receiver && handler.is_some();
    let is_axios = lower.starts_with("axios.");
    let known_consumer = is_axios
        || receiver.contains("client")
        || terminal_receiver == "api"
        || terminal_receiver == "http"
        || receiver.ends_with(".request");
    if !producer && !known_consumer {
        return None;
    }
    let path = target?;
    if producer && !is_relative_route(&path) {
        return None;
    }
    Some(HttpCall {
        node_kind: if producer { "endpoint" } else { "api" },
        edge_kind: if producer { "registers" } else { "requests" },
        label: bounded_api_label(&method, &path),
        confidence: if producer { 0.85 } else { 0.88 },
        metadata: HttpMetadata {
            role: if producer { "producer" } else { "consumer" },
            method: Some(method),
            path: Some(path),
            framework: is_axios.then_some("Axios"),
            source: target_node.map(|target| location(relative_path, target, source)),
            handler,
        },
    })
}

fn call_arguments(node: Node<'_>) -> Vec<Node<'_>> {
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Vec::new();
    };
    named_children(arguments)
}

fn fetch_method(arguments: &[Node<'_>], source: &str) -> Option<String> {
    let Some(options) = arguments.get(1).copied() else {
        return Some("GET".into());
    };
    method_from_options(options, source)
}

fn request_configuration<'tree>(
    arguments: &[Node<'tree>],
    source: &str,
    target_node: &mut Option<Node<'tree>>,
    target: &mut Option<String>,
) -> Option<String> {
    if let Some(configuration) = arguments
        .first()
        .copied()
        .filter(|node| node.kind() == "object")
    {
        if let Some(url_node) = object_property(configuration, "url", source) {
            *target_node = Some(url_node);
            *target = static_javascript_string(url_node, source)
                .and_then(|value| sanitized_http_target(&value));
        }
        return method_from_options(configuration, source);
    }
    let Some(options) = arguments.get(1).copied() else {
        return Some("GET".into());
    };
    method_from_options(options, source)
}

fn method_from_options(options: Node<'_>, source: &str) -> Option<String> {
    if options.kind() != "object" {
        return None;
    }
    if named_children(options).into_iter().any(|child| {
        child.kind() == "spread_element"
            || (child.kind() == "shorthand_property_identifier"
                && compact_text(child, source) == "method")
    }) {
        return None;
    }
    let Some(method_node) = object_property(options, "method", source) else {
        return Some("GET".into());
    };
    let method = static_javascript_string(method_node, source)?.to_ascii_lowercase();
    HTTP_METHODS
        .contains(&method.as_str())
        .then(|| method.to_ascii_uppercase())
}

fn object_property<'tree>(
    object: Node<'tree>,
    property: &str,
    source: &str,
) -> Option<Node<'tree>> {
    named_children(object).into_iter().find_map(|pair| {
        let key = pair.child_by_field_name("key")?;
        (compact_text(key, source).trim_matches(['\'', '"']) == property)
            .then(|| pair.child_by_field_name("value"))
            .flatten()
    })
}

fn route_handler(arguments: &[Node<'_>], source: &str, relative_path: &str) -> Option<HttpHandler> {
    let handler = arguments.get(1..)?.last().copied()?;
    let (kind, label, reference_only) = match handler.kind() {
        "arrow_function" | "function_expression" => ("handler", "inline handler".into(), false),
        "identifier" | "member_expression" | "subscript_expression" => {
            let label = compact_text(handler, source);
            if label.is_empty() || label.len() > MAX_LABEL_BYTES {
                return None;
            }
            ("handlerReference", label, true)
        }
        "object" => {
            let referenced = object_property(handler, "handler", source)?;
            let label = compact_text(referenced, source);
            if label.is_empty() || label.len() > MAX_LABEL_BYTES {
                return None;
            }
            return Some(HttpHandler {
                kind: "handlerReference",
                label,
                source: location(relative_path, referenced, source),
                ast_kind: referenced.kind().into(),
                reference_only: true,
            });
        }
        _ => return None,
    };
    Some(HttpHandler {
        kind,
        label,
        source: location(relative_path, handler, source),
        ast_kind: handler.kind().into(),
        reference_only,
    })
}

fn static_javascript_string(node: Node<'_>, source: &str) -> Option<String> {
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
    if raw.len() < 2 || raw.len() > MAX_STATIC_ROUTE_BYTES + 2 {
        return None;
    }
    let delimiter = *raw.as_bytes().first()?;
    if !matches!(delimiter, b'\'' | b'"' | b'`') || raw.as_bytes().last() != Some(&delimiter) {
        return None;
    }
    let value = &raw[1..raw.len() - 1];
    (!value.contains('\\') && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
