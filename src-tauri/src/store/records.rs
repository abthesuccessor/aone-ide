use std::collections::BTreeMap;

use rusqlite::{Connection, Row};
use serde_json::Value;

use crate::{
    domain::{CapabilityLevel, EvidenceKind, GraphEdge, GraphNode, SourceLocation, WorkspaceFile},
    error::AoneResult,
};

pub(super) fn count(connection: &Connection, table: &str) -> AoneResult<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&sql, [], |row| row.get::<_, i64>(0))? as usize)
}

pub(super) fn file_from_row(row: &Row<'_>) -> rusqlite::Result<WorkspaceFile> {
    Ok(WorkspaceFile {
        id: row.get(0)?,
        relative_path: row.get(1)?,
        language: row.get(2)?,
        capability: capability_from_db(&row.get::<_, String>(3)?),
        size_bytes: row.get::<_, i64>(4)? as u64,
        content_hash: row.get(5)?,
        modified_at: row.get(6)?,
        parse_errors: row.get(7)?,
    })
}

pub(super) fn node_from_row(row: &Row<'_>) -> rusqlite::Result<GraphNode> {
    let source_json: String = row.get(3)?;
    let metadata_json: String = row.get(6)?;
    Ok(GraphNode {
        id: row.get(0)?,
        kind: row.get(1)?,
        label: row.get(2)?,
        source: serde_json::from_str::<Option<SourceLocation>>(&source_json).unwrap_or_default(),
        language: row.get(4)?,
        evidence: evidence_from_db(&row.get::<_, String>(5)?),
        metadata: serde_json::from_str::<BTreeMap<String, Value>>(&metadata_json)
            .unwrap_or_default(),
    })
}

pub(super) fn edge_from_row(row: &Row<'_>) -> rusqlite::Result<GraphEdge> {
    let metadata_json: String = row.get(6)?;
    Ok(GraphEdge {
        id: row.get(0)?,
        source: row.get(1)?,
        target: row.get(2)?,
        kind: row.get(3)?,
        evidence: evidence_from_db(&row.get::<_, String>(4)?),
        confidence: row.get(5)?,
        metadata: serde_json::from_str::<BTreeMap<String, Value>>(&metadata_json)
            .unwrap_or_default(),
    })
}

pub(super) fn evidence_db(value: EvidenceKind) -> &'static str {
    match value {
        EvidenceKind::Declared => "declared",
        EvidenceKind::Resolved => "resolved",
        EvidenceKind::Observed => "observed",
        EvidenceKind::Inferred => "inferred",
    }
}

pub(super) fn evidence_from_db(value: &str) -> EvidenceKind {
    match value {
        "resolved" => EvidenceKind::Resolved,
        "observed" => EvidenceKind::Observed,
        "inferred" => EvidenceKind::Inferred,
        _ => EvidenceKind::Declared,
    }
}

pub(super) fn capability_db(value: CapabilityLevel) -> &'static str {
    match value {
        CapabilityLevel::TextOnly => "textOnly",
        CapabilityLevel::SyntaxOnly => "syntaxOnly",
        CapabilityLevel::Semantic => "semantic",
        CapabilityLevel::Runtime => "runtime",
    }
}

pub(super) fn capability_from_db(value: &str) -> CapabilityLevel {
    match value {
        "syntaxOnly" => CapabilityLevel::SyntaxOnly,
        "semantic" => CapabilityLevel::Semantic,
        "runtime" => CapabilityLevel::Runtime,
        _ => CapabilityLevel::TextOnly,
    }
}

pub(super) fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id TEXT NOT NULL UNIQUE,
    relative_path TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    capability TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    parse_errors INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS graph_nodes (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL REFERENCES files(relative_path) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    source_json TEXT NOT NULL,
    language TEXT,
    evidence TEXT NOT NULL,
    metadata_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graph_edges (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL REFERENCES files(relative_path) ON DELETE CASCADE,
    source TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
    target TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    evidence TEXT NOT NULL,
    confidence REAL,
    metadata_json TEXT NOT NULL
);

-- The source projection stores only the trigram index. Exact snippets are
-- securely reread from the selected workspace after candidates are found.
CREATE TABLE IF NOT EXISTS search_documents (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE REFERENCES files(relative_path) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS projection_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS source_search USING fts5(
    content,
    content='',
    contentless_delete=1,
    tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS search_documents_ad AFTER DELETE ON search_documents BEGIN
    DELETE FROM source_search WHERE rowid = old.rowid;
END;

CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);
CREATE INDEX IF NOT EXISTS idx_search_documents_path ON search_documents(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_file ON graph_nodes(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_kind_label ON graph_nodes(kind, label);
CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target);
CREATE INDEX IF NOT EXISTS idx_edges_kind ON graph_edges(kind);
"#;

pub(super) const SEMANTIC_SEARCH_SCHEMA: &str = r#"
-- Only declaration-like AST facts enter the semantic accelerator. High-volume
-- call targets and import text remain discoverable through source search.
CREATE TABLE semantic_search_documents (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id TEXT NOT NULL UNIQUE REFERENCES graph_nodes(id) ON DELETE CASCADE
);
CREATE VIRTUAL TABLE graph_search USING fts5(
    label,
    content='',
    contentless_delete=1,
    tokenize='trigram'
);
CREATE TRIGGER semantic_search_documents_ad
AFTER DELETE ON semantic_search_documents BEGIN
    DELETE FROM graph_search WHERE rowid = old.rowid;
END;
CREATE TRIGGER graph_nodes_ai AFTER INSERT ON graph_nodes
WHEN new.kind NOT IN ('file', 'callTarget', 'module') BEGIN
    INSERT INTO semantic_search_documents(node_id) VALUES (new.id);
    INSERT INTO graph_search(rowid, label) VALUES (last_insert_rowid(), new.label);
END;
CREATE TRIGGER graph_nodes_au AFTER UPDATE OF id, kind, label ON graph_nodes BEGIN
    DELETE FROM semantic_search_documents WHERE node_id = old.id;
    INSERT INTO semantic_search_documents(node_id)
    SELECT new.id WHERE new.kind NOT IN ('file', 'callTarget', 'module');
    INSERT INTO graph_search(rowid, label)
    SELECT rowid, new.label FROM semantic_search_documents WHERE node_id = new.id;
END;
CREATE INDEX idx_semantic_search_node ON semantic_search_documents(node_id);
"#;
