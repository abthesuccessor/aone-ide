use std::{collections::HashMap, time::Instant};

use serde_json::json;

use crate::{
    domain::{EvidenceKind, GraphEdge, GraphNode, SourceLocation},
    error::AoneResult,
};

use super::{
    AnalyzedSource,
    document_structure::{
        DocumentFormat, HeadingSpan, detect_headings, line_spans, mark_fenced_regions,
    },
    ids::stable_id,
};

pub(super) const MAX_DOCUMENT_FACTS_PER_FILE: usize = super::extract::MAX_FACTS_PER_FILE;
const MAX_DOCUMENT_LABEL_BYTES: usize = 240;

pub(super) fn extract_document(
    workspace_id: &str,
    relative_path: &str,
    source: &str,
    content_hash: &str,
    language_name: &'static str,
    file_node: GraphNode,
    deadline: Option<Instant>,
) -> AoneResult<AnalyzedSource> {
    let mut extractor = DocumentExtractor {
        workspace_id,
        relative_path,
        source,
        content_hash,
        language_name,
        format: DocumentFormat::from_path(relative_path),
        line_starts: line_starts(source),
        file_id: file_node.id.clone(),
        nodes: vec![file_node],
        edges: Vec::new(),
        occurrences: HashMap::new(),
        heading_stack: Vec::new(),
        last_segment_id: None,
        fact_count: 0,
        lines_visited: 0,
        segments_considered: 0,
        truncation_reasons: Vec::new(),
        deadline,
    };
    extractor.extract()?;
    extractor.record_file_metadata();
    let truncated = !extractor.truncation_reasons.is_empty();
    Ok(AnalyzedSource {
        nodes: extractor.nodes,
        edges: extractor.edges,
        parse_errors: truncated,
        fact_count: extractor.fact_count,
    })
}

struct DocumentExtractor<'a> {
    workspace_id: &'a str,
    relative_path: &'a str,
    source: &'a str,
    content_hash: &'a str,
    language_name: &'static str,
    format: DocumentFormat,
    line_starts: Vec<usize>,
    file_id: String,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    occurrences: HashMap<String, usize>,
    heading_stack: Vec<(usize, String)>,
    last_segment_id: Option<String>,
    fact_count: usize,
    lines_visited: usize,
    segments_considered: usize,
    truncation_reasons: Vec<&'static str>,
    deadline: Option<Instant>,
}

impl DocumentExtractor<'_> {
    fn extract(&mut self) -> AoneResult<()> {
        let (mut lines, lines_truncated) = line_spans(self.source);
        if lines_truncated {
            self.mark_truncated("document line limit reached");
        }
        mark_fenced_regions(self.source, &mut lines, self.format);
        let headings = detect_headings(self.source, &mut lines, self.format);
        let mut paragraph = None::<(usize, usize)>;
        let mut stopped_early = false;

        for (index, line) in lines.iter().enumerate() {
            super::deadline::enforce_analysis_deadline(self.deadline)?;
            self.lines_visited = self.lines_visited.saturating_add(1);
            if let Some(heading) = headings[index].as_ref() {
                if let Some((start, end)) = paragraph.take()
                    && !self.emit_sentences(start, end)?
                {
                    stopped_early = true;
                    break;
                }
                if !self.emit_heading(heading)? {
                    stopped_early = true;
                    break;
                }
                continue;
            }
            if line.excluded {
                if let Some((start, end)) = paragraph.take()
                    && !self.emit_sentences(start, end)?
                {
                    stopped_early = true;
                    break;
                }
                continue;
            }
            let Some((start, end)) = trimmed_span(self.source, line.start, line.end) else {
                if let Some((start, end)) = paragraph.take()
                    && !self.emit_sentences(start, end)?
                {
                    stopped_early = true;
                    break;
                }
                continue;
            };
            paragraph = Some(match paragraph {
                Some((paragraph_start, _)) => (paragraph_start, end),
                None => (start, end),
            });
        }

        if !stopped_early && let Some((start, end)) = paragraph {
            self.emit_sentences(start, end)?;
        }
        Ok(())
    }

    fn emit_heading(&mut self, heading: &HeadingSpan) -> AoneResult<bool> {
        super::deadline::enforce_analysis_deadline(self.deadline)?;
        while self
            .heading_stack
            .last()
            .is_some_and(|(level, _)| *level >= heading.level)
        {
            self.heading_stack.pop();
        }
        let owner_id = self
            .heading_stack
            .last()
            .map_or_else(|| self.file_id.clone(), |(_, id)| id.clone());
        let Some(node_id) = self.emit_segment(
            "heading",
            heading.start,
            heading.end,
            &owner_id,
            Some((heading.level, heading.style)),
        ) else {
            return Ok(false);
        };
        self.heading_stack.push((heading.level, node_id));
        Ok(true)
    }

    fn emit_sentences(&mut self, start: usize, end: usize) -> AoneResult<bool> {
        let owner_id = self
            .heading_stack
            .last()
            .map_or_else(|| self.file_id.clone(), |(_, id)| id.clone());
        let remaining = MAX_DOCUMENT_FACTS_PER_FILE
            .saturating_sub(self.fact_count)
            .saturating_add(1);
        for (sentence_start, sentence_end) in sentence_spans(self.source, start, end, remaining) {
            super::deadline::enforce_analysis_deadline(self.deadline)?;
            if self
                .emit_segment("sentence", sentence_start, sentence_end, &owner_id, None)
                .is_none()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn emit_segment(
        &mut self,
        kind: &str,
        start: usize,
        end: usize,
        owner_id: &str,
        heading: Option<(usize, &'static str)>,
    ) -> Option<String> {
        self.segments_considered = self.segments_considered.saturating_add(1);
        if self.fact_count >= MAX_DOCUMENT_FACTS_PER_FILE {
            self.mark_truncated("document fact limit reached");
            return None;
        }
        let exact = &self.source[start..end];
        let (label, label_truncated) = compact_document_text(exact);
        if label.is_empty() {
            return Some(owner_id.to_string());
        }
        let content_id = stable_id("document-content", &[exact]);
        let semantic_id = stable_id(
            "document-segment-semantic",
            &[
                self.workspace_id,
                self.relative_path,
                self.language_name,
                kind,
                owner_id,
                &content_id,
            ],
        );
        let occurrence = self.occurrences.entry(semantic_id.clone()).or_default();
        let ordinal = occurrence.to_string();
        *occurrence = occurrence.saturating_add(1);
        let node_id = stable_id("document-segment", &[&semantic_id, &ordinal]);
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("contentHash".into(), json!(self.content_hash));
        metadata.insert("documentFormat".into(), json!(self.format.name()));
        metadata.insert("documentOrder".into(), json!(self.fact_count));
        metadata.insert("labelTruncated".into(), json!(label_truncated));
        metadata.insert("parser".into(), json!("aone-document"));
        metadata.insert("parserVersion".into(), json!(1));
        if let Some((level, style)) = heading {
            metadata.insert("headingLevel".into(), json!(level));
            metadata.insert("headingStyle".into(), json!(style));
        }
        self.nodes.push(GraphNode {
            id: node_id.clone(),
            kind: kind.into(),
            label,
            source: Some(self.location(start, end)),
            language: Some(self.language_name.into()),
            evidence: EvidenceKind::Declared,
            metadata,
        });
        self.edges.push(graph_edge(owner_id, &node_id, "contains"));
        if let Some(previous_id) = self.last_segment_id.as_deref() {
            self.edges
                .push(graph_edge(previous_id, &node_id, "precedes"));
        }
        self.last_segment_id = Some(node_id.clone());
        self.fact_count = self.fact_count.saturating_add(1);
        Some(node_id)
    }

    fn location(&self, start: usize, end: usize) -> SourceLocation {
        let (start_line, start_column) = line_column(self.source, &self.line_starts, start);
        let (end_line, end_column) = line_column(self.source, &self.line_starts, end);
        SourceLocation {
            relative_path: self.relative_path.into(),
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    fn mark_truncated(&mut self, reason: &'static str) {
        if !self.truncation_reasons.contains(&reason) {
            self.truncation_reasons.push(reason);
        }
    }

    fn record_file_metadata(&mut self) {
        let truncated = !self.truncation_reasons.is_empty();
        if let Some(file_node) = self.nodes.first_mut() {
            file_node
                .metadata
                .insert("analysisTruncated".into(), json!(truncated));
            file_node
                .metadata
                .insert("documentFormat".into(), json!(self.format.name()));
            file_node
                .metadata
                .insert("documentLinesVisited".into(), json!(self.lines_visited));
            file_node.metadata.insert(
                "documentSegmentsConsidered".into(),
                json!(self.segments_considered),
            );
            file_node
                .metadata
                .insert("factsExtracted".into(), json!(self.fact_count));
            file_node
                .metadata
                .insert("parser".into(), json!("aone-document"));
            file_node.metadata.insert("parserVersion".into(), json!(1));
            if truncated {
                file_node.metadata.insert(
                    "analysisTruncationReasons".into(),
                    json!(self.truncation_reasons),
                );
            }
        }
    }
}

fn sentence_spans(source: &str, start: usize, end: usize, maximum: usize) -> Vec<(usize, usize)> {
    let mut sentences = Vec::new();
    let mut sentence_start = start;
    let mut iterator = source[start..end].char_indices().peekable();
    while let Some((relative, character)) = iterator.next() {
        if !matches!(character, '.' | '!' | '?') {
            continue;
        }
        let punctuation_end = start + relative + character.len_utf8();
        if character == '.' && is_abbreviation(source, sentence_start, punctuation_end) {
            continue;
        }
        let mut candidate_end = punctuation_end;
        while let Some((next_relative, next)) = iterator.peek().copied() {
            if !matches!(next, '"' | '\'' | ')' | ']' | '}' | '”' | '’') {
                break;
            }
            iterator.next();
            candidate_end = start + next_relative + next.len_utf8();
        }
        if candidate_end < end
            && !source[candidate_end..end]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            continue;
        }
        if let Some(span) = trimmed_span(source, sentence_start, candidate_end) {
            sentences.push(span);
            if sentences.len() >= maximum {
                return sentences;
            }
        }
        sentence_start = candidate_end;
    }
    if sentences.len() < maximum
        && let Some(span) = trimmed_span(source, sentence_start, end)
    {
        sentences.push(span);
    }
    sentences
}

fn is_abbreviation(source: &str, sentence_start: usize, punctuation_end: usize) -> bool {
    let prefix = &source[sentence_start..punctuation_end.saturating_sub(1)];
    let token = prefix
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .trim_matches(|character: char| !character.is_alphanumeric() && character != '.');
    matches!(
        token.to_ascii_lowercase().as_str(),
        "e.g" | "i.e" | "mr" | "mrs" | "ms" | "dr" | "prof" | "sr" | "jr" | "vs"
    )
}

fn trimmed_span(source: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let value = source.get(start..end)?;
    let trimmed_start = value.trim_start();
    let adjusted_start = start + value.len().saturating_sub(trimmed_start.len());
    let trimmed = trimmed_start.trim_end();
    let adjusted_end = adjusted_start + trimmed.len();
    (adjusted_start < adjusted_end).then_some((adjusted_start, adjusted_end))
}

fn compact_document_text(value: &str) -> (String, bool) {
    let mut output = String::new();
    let mut pending_space = false;
    let mut truncated = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space |= !output.is_empty();
            continue;
        }
        let additional = character.len_utf8() + usize::from(pending_space);
        if output.len().saturating_add(additional) > MAX_DOCUMENT_LABEL_BYTES {
            truncated = true;
            break;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    if truncated {
        output.push('…');
    }
    (output, truncated)
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

fn line_column(source: &str, line_starts: &[usize], offset: usize) -> (usize, usize) {
    let line_index = line_starts.partition_point(|line_start| *line_start <= offset) - 1;
    let line_start = line_starts[line_index];
    let column = source[line_start..offset].encode_utf16().count() + 1;
    (line_index + 1, column)
}

fn graph_edge(source: &str, target: &str, kind: &str) -> GraphEdge {
    GraphEdge {
        id: stable_id("edge", &[source, target, kind]),
        source: source.into(),
        target: target.into(),
        kind: kind.into(),
        evidence: EvidenceKind::Declared,
        confidence: None,
        metadata: std::collections::BTreeMap::new(),
    }
}
