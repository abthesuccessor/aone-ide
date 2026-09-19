use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    rc::Rc,
    time::Instant,
};

use serde_json::json;
use tree_sitter::{Node, Parser};

use crate::{
    domain::{EvidenceKind, GraphEdge, GraphNode, SourceLocation},
    error::{AoneError, AoneResult},
};

use super::{
    AnalyzedSource,
    api_definition::{DefinitionHttpIdentity, definition_http_facts},
    api_handler::{HttpHandlerIdentity, handler_fact},
    document::extract_document,
    extract_support::{VisitFrame, graph_edge},
    ids::{EventIdentity, stable_event_id, stable_id},
    language::language_support,
    openapi::extract_openapi,
    syntax::{
        call_name, classify_definition, definition_kind, definition_signature, is_call, is_import,
        location, node_name, semantic_call,
    },
    text::compact_text,
    types::ParserKind,
};

pub(super) const MAX_FACTS_PER_FILE: usize = 5_000;
pub(super) const MAX_AST_DEPTH: usize = 512;
const MAX_AST_NODE_VISITS: usize = 250_000;
pub(crate) fn analyze_source_with_deadline(
    workspace_id: &str,
    relative_path: &str,
    source: &str,
    content_hash: &str,
    deadline: Option<Instant>,
) -> AoneResult<AnalyzedSource> {
    super::deadline::enforce_analysis_deadline(deadline)?;
    let support = language_support(Path::new(relative_path));
    let file_id = stable_id("file", &[workspace_id, relative_path]);
    let mut file_metadata = BTreeMap::new();
    file_metadata.insert("contentHash".into(), json!(content_hash));
    file_metadata.insert("capability".into(), json!(support.capability));
    file_metadata.insert("analysisTruncated".into(), json!(false));
    file_metadata.insert("astNodesVisited".into(), json!(0));
    file_metadata.insert("factsExtracted".into(), json!(0));

    let file_node = GraphNode {
        id: file_id.clone(),
        kind: "file".into(),
        label: relative_path.into(),
        source: Some(SourceLocation {
            relative_path: relative_path.into(),
            start_line: 1,
            start_column: 1,
            end_line: source.lines().count().max(1),
            end_column: 1,
        }),
        language: Some(support.name.into()),
        evidence: EvidenceKind::Declared,
        metadata: file_metadata,
    };
    super::deadline::enforce_analysis_deadline(deadline)?;

    if let Some(openapi) = extract_openapi(
        workspace_id,
        relative_path,
        source,
        content_hash,
        file_node.clone(),
        deadline,
    )? {
        return Ok(openapi);
    }

    if matches!(support.parser, ParserKind::Document) {
        return extract_document(
            workspace_id,
            relative_path,
            source,
            content_hash,
            support.name,
            file_node,
            deadline,
        );
    }

    let Some(language) = support.parser.language() else {
        return Ok(AnalyzedSource {
            nodes: vec![file_node],
            edges: Vec::new(),
            parse_errors: false,
            fact_count: 0,
        });
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| AoneError::Parser(error.to_string()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AoneError::Parser("Tree-sitter returned no syntax tree".into()))?;
    super::deadline::enforce_analysis_deadline(deadline)?;

    let mut extractor = Extractor {
        workspace_id,
        relative_path,
        source,
        content_hash,
        language_name: support.name,
        file_id: file_id.clone(),
        nodes: vec![file_node],
        edges: Vec::new(),
        definition_occurrences: HashMap::new(),
        event_occurrences: HashMap::new(),
        fact_count: 0,
        visited_node_count: 0,
        truncation_reasons: Vec::new(),
        deadline,
    };
    let root = tree.root_node();
    let has_parse_errors = root.has_error();
    extractor.extract(root, &file_id)?;
    extractor.record_analysis_metadata();
    let truncated = !extractor.truncation_reasons.is_empty();

    Ok(AnalyzedSource {
        nodes: extractor.nodes,
        edges: extractor.edges,
        parse_errors: has_parse_errors || truncated,
        fact_count: extractor.fact_count,
    })
}

struct Extractor<'a> {
    workspace_id: &'a str,
    relative_path: &'a str,
    source: &'a str,
    content_hash: &'a str,
    language_name: &'static str,
    file_id: String,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    definition_occurrences: HashMap<String, usize>,
    event_occurrences: HashMap<String, usize>,
    fact_count: usize,
    visited_node_count: usize,
    truncation_reasons: Vec<&'static str>,
    deadline: Option<Instant>,
}

impl Extractor<'_> {
    fn extract<'tree>(&mut self, root: Node<'tree>, file_id: &str) -> AoneResult<()> {
        let mut stack = vec![VisitFrame {
            node: root,
            owner_id: Rc::from(file_id),
            scope: Rc::from("file"),
            sibling_ordinal: 0,
            depth: 0,
        }];

        while let Some(frame) = stack.pop() {
            super::deadline::enforce_analysis_deadline(self.deadline)?;
            if self.visited_node_count >= MAX_AST_NODE_VISITS {
                self.mark_truncated("AST node visit limit reached");
                break;
            }
            if frame.depth > MAX_AST_DEPTH {
                self.mark_truncated("AST depth limit reached");
                continue;
            }

            self.visited_node_count += 1;
            let node = frame.node;
            let owner_id = frame.owner_id;
            let scope = frame.scope;
            let mut child_owner = Rc::clone(&owner_id);
            let mut child_scope = Rc::clone(&scope);
            let mut identifier_search_truncated = false;

            if let Some(base_kind) =
                definition_kind(node, self.source, &mut identifier_search_truncated)
            {
                let label = node_name(node, self.source, &mut identifier_search_truncated)
                    .unwrap_or_else(|| {
                        format!("{}@{}", node.kind(), node.start_position().row + 1)
                    });
                let (kind, evidence, role) =
                    classify_definition(base_kind, &label, self.language_name);
                let qualified = if scope.as_ref() == "file" {
                    label.clone()
                } else {
                    format!("{scope}::{label}")
                };
                let node_id = self.definition_id(node, kind, &qualified, &owner_id);
                self.nodes
                    .push(self.graph_node(node_id.clone(), kind, label, node, evidence));
                if let Some(role) = role
                    && let Some(created) = self.nodes.last_mut()
                {
                    created.metadata.insert("roleHeuristic".into(), json!(role));
                    created.metadata.insert("confidence".into(), json!(0.8));
                }
                self.edges.push(graph_edge(
                    &owner_id,
                    &node_id,
                    "contains",
                    EvidenceKind::Declared,
                    None,
                ));
                self.fact_count += 1;
                if matches!(kind, "function" | "method") {
                    let identity = DefinitionHttpIdentity {
                        workspace_id: self.workspace_id,
                        relative_path: self.relative_path,
                        content_hash: self.content_hash,
                        language_name: self.language_name,
                        owner_id: &node_id,
                    };
                    for (endpoint, edge) in definition_http_facts(identity, node, self.source) {
                        self.nodes.push(endpoint);
                        self.edges.push(edge);
                        self.fact_count += 1;
                    }
                }
                child_owner = Rc::from(node_id);
                child_scope = Rc::from(qualified);
            } else if is_import(node.kind()) {
                let label = compact_text(node, self.source);
                if !label.is_empty() {
                    let node_id =
                        self.ast_id(node, "module", &label, frame.sibling_ordinal, &self.file_id);
                    self.nodes.push(self.graph_node(
                        node_id.clone(),
                        "module",
                        label,
                        node,
                        EvidenceKind::Declared,
                    ));
                    self.edges.push(graph_edge(
                        &self.file_id,
                        &node_id,
                        "imports",
                        EvidenceKind::Declared,
                        None,
                    ));
                    self.fact_count += 1;
                }
            } else if is_call(node.kind()) {
                self.extract_call(node, &owner_id, frame.sibling_ordinal);
            }

            if identifier_search_truncated {
                self.mark_truncated("identifier search limit reached");
            }

            let child_count = node.named_child_count();
            if self.fact_count >= MAX_FACTS_PER_FILE {
                if child_count > 0 || !stack.is_empty() {
                    self.mark_truncated("per-file fact limit reached");
                }
                break;
            }
            if frame.depth >= MAX_AST_DEPTH {
                if child_count > 0 {
                    self.mark_truncated("AST depth limit reached");
                }
                continue;
            }

            let remaining_capacity = MAX_AST_NODE_VISITS
                .saturating_sub(self.visited_node_count)
                .saturating_sub(stack.len());
            let scheduled_children = child_count.min(remaining_capacity);
            if scheduled_children < child_count {
                self.mark_truncated("AST node visit limit reached");
            }
            for index in (0..scheduled_children).rev() {
                let Ok(child_index) = u32::try_from(index) else {
                    self.mark_truncated("AST child index limit reached");
                    break;
                };
                if let Some(child) = node.named_child(child_index) {
                    stack.push(VisitFrame {
                        node: child,
                        owner_id: Rc::clone(&child_owner),
                        scope: Rc::clone(&child_scope),
                        sibling_ordinal: index,
                        depth: frame.depth + 1,
                    });
                }
            }
        }
        Ok(())
    }

    fn extract_call(&mut self, node: Node<'_>, owner_id: &str, sibling_ordinal: usize) {
        let callee = call_name(node, self.source);
        let semantic_fact =
            semantic_call(node, self.source, self.language_name, self.relative_path);
        let label = semantic_fact
            .as_ref()
            .map(|fact| fact.label.clone())
            .or_else(|| callee.clone())
            .unwrap_or_else(|| compact_text(node, self.source));
        if label.is_empty() {
            return;
        }
        let node_kind = semantic_fact
            .as_ref()
            .map_or("callTarget", |fact| fact.node_kind);
        let edge_kind = semantic_fact
            .as_ref()
            .map_or("calls", |fact| fact.edge_kind);
        let confidence = semantic_fact.as_ref().map_or(0.55, |fact| fact.confidence);
        let event = semantic_fact.as_ref().and_then(|fact| fact.event.as_ref());
        let node_id = if let Some(event) = event {
            stable_event_id(
                EventIdentity {
                    workspace_id: self.workspace_id,
                    relative_path: self.relative_path,
                    language: self.language_name,
                    owner_id,
                    callee: callee.as_deref().unwrap_or(&event.operation),
                    operation: &event.operation,
                    direction: event.direction,
                    event_name: &label,
                },
                &mut self.event_occurrences,
            )
        } else {
            self.ast_id(node, node_kind, &label, sibling_ordinal, owner_id)
        };
        let mut graph_node = self.graph_node(
            node_id.clone(),
            node_kind,
            label,
            node,
            EvidenceKind::Inferred,
        );
        if let Some(fact) = semantic_fact.as_ref()
            && let Some(event) = fact.event.as_ref()
        {
            event.decorate(&mut graph_node, fact.confidence);
        }
        let http = semantic_fact.as_ref().and_then(|fact| fact.http.as_ref());
        if let Some(http) = http {
            http.decorate(&mut graph_node);
        }
        self.nodes.push(graph_node);
        if let Some(handler) = http.and_then(|metadata| metadata.handler.as_ref()) {
            if self.fact_count.saturating_add(1) < MAX_FACTS_PER_FILE {
                let (handler_node, handler_edge) = handler_fact(
                    HttpHandlerIdentity {
                        workspace_id: self.workspace_id,
                        relative_path: self.relative_path,
                        content_hash: self.content_hash,
                        language_name: self.language_name,
                        endpoint_id: &node_id,
                    },
                    handler,
                    confidence,
                );
                self.nodes.push(handler_node);
                self.edges.push(handler_edge);
                self.fact_count += 1;
            } else {
                self.mark_truncated("per-file fact limit reached");
            }
        }
        self.edges.push(graph_edge(
            owner_id,
            &node_id,
            edge_kind,
            EvidenceKind::Inferred,
            Some(confidence),
        ));
        self.fact_count += 1;
    }

    fn mark_truncated(&mut self, reason: &'static str) {
        if !self.truncation_reasons.contains(&reason) {
            self.truncation_reasons.push(reason);
        }
    }

    fn record_analysis_metadata(&mut self) {
        let truncated = !self.truncation_reasons.is_empty();
        let reasons = self.truncation_reasons.clone();
        if let Some(file_node) = self.nodes.first_mut() {
            file_node
                .metadata
                .insert("analysisTruncated".into(), json!(truncated));
            file_node
                .metadata
                .insert("astNodesVisited".into(), json!(self.visited_node_count));
            file_node
                .metadata
                .insert("factsExtracted".into(), json!(self.fact_count));
            if truncated {
                file_node
                    .metadata
                    .insert("analysisTruncationReasons".into(), json!(reasons));
            }
        }
    }

    fn definition_id(
        &mut self,
        node: Node<'_>,
        kind: &str,
        qualified: &str,
        owner_id: &str,
    ) -> String {
        let signature = definition_signature(node, self.source);
        let semantic_identity = stable_id(
            "symbol-semantic",
            &[
                self.workspace_id,
                self.relative_path,
                self.language_name,
                kind,
                qualified,
                &signature,
                owner_id,
            ],
        );
        let occurrence = self
            .definition_occurrences
            .entry(semantic_identity.clone())
            .or_default();
        let occurrence = {
            let current = *occurrence;
            *occurrence = occurrence.saturating_add(1);
            current.to_string()
        };
        stable_id("symbol", &[&semantic_identity, &occurrence])
    }

    fn ast_id(
        &self,
        node: Node<'_>,
        kind: &str,
        label: &str,
        ordinal: usize,
        owner_id: &str,
    ) -> String {
        stable_id(
            "ast",
            &[
                self.workspace_id,
                self.relative_path,
                self.language_name,
                self.content_hash,
                kind,
                label,
                owner_id,
                node.kind(),
                &ordinal.to_string(),
                &node.start_byte().to_string(),
                &node.end_byte().to_string(),
            ],
        )
    }

    fn graph_node(
        &self,
        id: String,
        kind: &str,
        label: String,
        node: Node<'_>,
        evidence: EvidenceKind,
    ) -> GraphNode {
        let mut metadata = BTreeMap::new();
        metadata.insert("astKind".into(), json!(node.kind()));
        metadata.insert("contentHash".into(), json!(self.content_hash));
        metadata.insert("parser".into(), json!("tree-sitter"));
        GraphNode {
            id,
            kind: kind.into(),
            label,
            source: Some(location(self.relative_path, node, self.source)),
            language: Some(self.language_name.into()),
            evidence,
            metadata,
        }
    }
}
