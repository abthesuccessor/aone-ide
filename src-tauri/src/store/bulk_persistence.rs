use rusqlite::{Statement, Transaction, params};

#[cfg(test)]
use std::cell::Cell;

use super::records::{capability_db, evidence_db};
use crate::{error::AoneResult, scanner::IndexedFile};

const SUSPEND_BULK_PROJECTIONS: &str = r#"
DROP TRIGGER IF EXISTS search_documents_ad;
DROP TRIGGER IF EXISTS semantic_search_documents_ad;
DROP TRIGGER IF EXISTS graph_nodes_ai;
DROP TRIGGER IF EXISTS graph_nodes_au;
DROP INDEX IF EXISTS idx_files_language;
DROP INDEX IF EXISTS idx_search_documents_path;
DROP INDEX IF EXISTS idx_nodes_file;
DROP INDEX IF EXISTS idx_nodes_kind_label;
DROP INDEX IF EXISTS idx_edges_source;
DROP INDEX IF EXISTS idx_edges_target;
DROP INDEX IF EXISTS idx_edges_kind;
DROP INDEX IF EXISTS idx_semantic_search_node;
"#;

const CLEAR_BULK_DATA: &str = r#"
INSERT INTO source_search(source_search) VALUES('delete-all');
INSERT INTO graph_search(graph_search) VALUES('delete-all');
DELETE FROM graph_edges;
DELETE FROM semantic_search_documents;
DELETE FROM graph_nodes;
DELETE FROM search_documents;
DELETE FROM files;
"#;

const RESTORE_BULK_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);
CREATE INDEX IF NOT EXISTS idx_search_documents_path ON search_documents(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_file ON graph_nodes(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_kind_label ON graph_nodes(kind, label);
CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target);
CREATE INDEX IF NOT EXISTS idx_edges_kind ON graph_edges(kind);
CREATE INDEX IF NOT EXISTS idx_semantic_search_node ON semantic_search_documents(node_id);
"#;

const RESTORE_SEARCH_TRIGGERS: &str = r#"
CREATE TRIGGER IF NOT EXISTS search_documents_ad AFTER DELETE ON search_documents BEGIN
    DELETE FROM source_search WHERE rowid = old.rowid;
END;
CREATE TRIGGER IF NOT EXISTS semantic_search_documents_ad
AFTER DELETE ON semantic_search_documents BEGIN
    DELETE FROM graph_search WHERE rowid = old.rowid;
END;
CREATE TRIGGER IF NOT EXISTS graph_nodes_ai AFTER INSERT ON graph_nodes
WHEN new.kind NOT IN ('file', 'callTarget', 'module') BEGIN
    INSERT INTO semantic_search_documents(node_id) VALUES (new.id);
    INSERT INTO graph_search(rowid, label) VALUES (last_insert_rowid(), new.label);
END;
CREATE TRIGGER IF NOT EXISTS graph_nodes_au AFTER UPDATE OF id, kind, label ON graph_nodes BEGIN
    DELETE FROM semantic_search_documents WHERE node_id = old.id;
    INSERT INTO semantic_search_documents(node_id)
    SELECT new.id WHERE new.kind NOT IN ('file', 'callTarget', 'module');
    INSERT INTO graph_search(rowid, label)
    SELECT rowid, new.label FROM semantic_search_documents WHERE node_id = new.id;
END;
"#;

#[cfg(test)]
thread_local! {
    static BULK_PREPARE_PASSES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_bulk_prepare_passes() {
    BULK_PREPARE_PASSES.set(0);
}

#[cfg(test)]
pub(super) fn bulk_prepare_passes() -> usize {
    BULK_PREPARE_PASSES.get()
}

pub(super) fn replace_workspace_files(
    transaction: &Transaction<'_>,
    files: &[IndexedFile],
) -> AoneResult<()> {
    transaction.execute_batch(SUSPEND_BULK_PROJECTIONS)?;
    transaction.execute_batch(CLEAR_BULK_DATA)?;
    {
        let mut statements = BulkInsertStatements::prepare(transaction)?;
        for file in files {
            statements.insert(file)?;
        }
    }
    transaction.execute(
        "INSERT INTO semantic_search_documents(node_id)
         SELECT id FROM graph_nodes
         WHERE kind NOT IN ('file', 'callTarget', 'module')
         ORDER BY id",
        [],
    )?;
    transaction.execute(
        "INSERT INTO graph_search(rowid, label)
         SELECT d.rowid, g.label
         FROM semantic_search_documents d
         JOIN graph_nodes g ON g.id = d.node_id",
        [],
    )?;
    Ok(())
}

pub(super) fn restore_bulk_indexes_and_search_triggers(
    transaction: &Transaction<'_>,
) -> AoneResult<()> {
    transaction.execute_batch(RESTORE_BULK_INDEXES)?;
    transaction.execute_batch(RESTORE_SEARCH_TRIGGERS)?;
    Ok(())
}

struct BulkInsertStatements<'connection> {
    file: Statement<'connection>,
    search_document: Statement<'connection>,
    source_search: Statement<'connection>,
    graph_node: Statement<'connection>,
    graph_edge: Statement<'connection>,
}

impl<'connection> BulkInsertStatements<'connection> {
    fn prepare(transaction: &'connection Transaction<'_>) -> AoneResult<Self> {
        #[cfg(test)]
        BULK_PREPARE_PASSES.set(BULK_PREPARE_PASSES.get() + 1);
        Ok(Self {
            file: transaction.prepare(
                "INSERT INTO files
                 (id, relative_path, language, capability, size_bytes, content_hash, modified_at, parse_errors)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
            search_document: transaction.prepare(
                "INSERT INTO search_documents (file_path) VALUES (?1) RETURNING rowid",
            )?,
            source_search: transaction
                .prepare("INSERT INTO source_search (rowid, content) VALUES (?1, ?2)")?,
            graph_node: transaction.prepare(
                "INSERT INTO graph_nodes
                 (id, file_path, kind, label, source_json, language, evidence, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
            graph_edge: transaction.prepare(
                "INSERT INTO graph_edges
                 (id, file_path, source, target, kind, evidence, confidence, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
        })
    }

    fn insert(&mut self, indexed: &IndexedFile) -> AoneResult<()> {
        let file = &indexed.file;
        self.file.execute(params![
            file.id,
            file.relative_path,
            file.language,
            capability_db(file.capability),
            file.size_bytes as i64,
            file.content_hash,
            file.modified_at,
            file.parse_errors,
        ])?;
        let search_row_id = self
            .search_document
            .query_row(params![file.relative_path], |row| row.get::<_, i64>(0))?;
        self.source_search
            .execute(params![search_row_id, indexed.source])?;
        for node in &indexed.analysis.nodes {
            let source_json = serde_json::to_string(&node.source)?;
            let metadata_json = serde_json::to_string(&node.metadata)?;
            self.graph_node.execute(params![
                node.id,
                file.relative_path,
                node.kind,
                node.label,
                source_json,
                node.language,
                evidence_db(node.evidence),
                metadata_json,
            ])?;
        }
        for edge in &indexed.analysis.edges {
            let metadata_json = serde_json::to_string(&edge.metadata)?;
            self.graph_edge.execute(params![
                edge.id,
                file.relative_path,
                edge.source,
                edge.target,
                edge.kind,
                evidence_db(edge.evidence),
                edge.confidence,
                metadata_json,
            ])?;
        }
        Ok(())
    }
}
