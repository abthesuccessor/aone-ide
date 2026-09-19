use rusqlite::{OptionalExtension, params};

use super::GraphStore;
use crate::{
    domain::SourceLocation,
    error::{AoneError, AoneResult},
};

pub(crate) const MAX_SOURCE_CANDIDATES: usize = 2_000;
pub(crate) const MAX_SHORT_QUERY_CANDIDATES: usize = 4_000;
pub(crate) const MAX_PATH_CANDIDATES: usize = 200;
pub(crate) const MAX_SEMANTIC_CANDIDATES: usize = 1_000;

#[derive(Debug, Clone)]
pub(crate) struct SourceSearchCandidate {
    pub relative_path: String,
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticSearchCandidate {
    pub node_id: String,
    pub relative_path: String,
    pub kind: String,
    pub label: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub source: SourceLocation,
}

#[derive(Debug, Clone)]
pub(crate) struct SearchCandidates {
    pub indexed_file_count: usize,
    pub source_files: Vec<SourceSearchCandidate>,
    pub path_files: Vec<SourceSearchCandidate>,
    pub semantic_nodes: Vec<SemanticSearchCandidate>,
    pub truncated: bool,
}

impl GraphStore {
    pub(crate) fn search_candidates(&self, query: &str) -> AoneResult<SearchCandidates> {
        let indexed_file_count = self.counts()?.0;
        let (source_files, source_truncated) = self.source_candidates(query)?;
        let (path_files, path_truncated) = self.path_candidates(query)?;
        let (semantic_nodes, semantic_truncated) = self.semantic_candidates(query)?;
        Ok(SearchCandidates {
            indexed_file_count,
            source_files,
            path_files,
            semantic_nodes,
            truncated: source_truncated || path_truncated || semantic_truncated,
        })
    }

    fn source_candidates(&self, query: &str) -> AoneResult<(Vec<SourceSearchCandidate>, bool)> {
        if query.chars().count() < 3 {
            return self.short_query_source_candidates();
        }
        let mut statement = self.connection.prepare(
            "SELECT d.file_path, f.content_hash, f.size_bytes
             FROM source_search
             JOIN search_documents d ON d.rowid = source_search.rowid
             JOIN files f ON f.relative_path = d.file_path
             WHERE source_search MATCH ?1
             ORDER BY bm25(source_search), d.file_path
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![fts_literal(query), MAX_SOURCE_CANDIDATES as i64 + 1],
            source_candidate_from_row,
        )?;
        bounded_collect(rows, MAX_SOURCE_CANDIDATES)
    }

    fn short_query_source_candidates(&self) -> AoneResult<(Vec<SourceSearchCandidate>, bool)> {
        let mut statement = self.connection.prepare(
            "SELECT d.file_path, f.content_hash, f.size_bytes
             FROM search_documents d
             JOIN files f ON f.relative_path = d.file_path
             ORDER BY d.file_path
             LIMIT ?1",
        )?;
        let rows = statement.query_map(
            params![MAX_SHORT_QUERY_CANDIDATES as i64 + 1],
            source_candidate_from_row,
        )?;
        bounded_collect(rows, MAX_SHORT_QUERY_CANDIDATES)
    }

    fn path_candidates(&self, query: &str) -> AoneResult<(Vec<SourceSearchCandidate>, bool)> {
        let pattern = format!("%{}%", super::records::escape_like(query));
        let mut statement = self.connection.prepare(
            "SELECT relative_path, content_hash, size_bytes FROM files
             WHERE relative_path LIKE ?1 ESCAPE '\\' COLLATE NOCASE
             ORDER BY CASE WHEN relative_path = ?2 COLLATE NOCASE THEN 0 ELSE 1 END,
                      relative_path COLLATE NOCASE
             LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![pattern, query, MAX_PATH_CANDIDATES as i64 + 1],
            source_candidate_from_row,
        )?;
        bounded_collect(rows, MAX_PATH_CANDIDATES)
    }

    fn semantic_candidates(&self, query: &str) -> AoneResult<(Vec<SemanticSearchCandidate>, bool)> {
        let (sql, search_value) = if query.chars().count() >= 3 {
            (
                "SELECT g.id, g.file_path, g.kind, g.label, g.source_json, f.content_hash,
                        f.size_bytes
                 FROM graph_search
                 JOIN semantic_search_documents d ON d.rowid = graph_search.rowid
                 JOIN graph_nodes g ON g.id = d.node_id
                 JOIN files f ON f.relative_path = g.file_path
                 WHERE graph_search MATCH ?1
                 ORDER BY bm25(graph_search), g.file_path, g.id
                 LIMIT ?2",
                fts_literal(query),
            )
        } else {
            (
                "SELECT g.id, g.file_path, g.kind, g.label, g.source_json, f.content_hash,
                        f.size_bytes
                 FROM graph_nodes g
                 JOIN semantic_search_documents d ON d.node_id = g.id
                 JOIN files f ON f.relative_path = g.file_path
                 WHERE g.label LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                 ORDER BY g.file_path, g.id
                 LIMIT ?2",
                format!("%{}%", super::records::escape_like(query)),
            )
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(
            params![search_value, MAX_SEMANTIC_CANDIDATES as i64 + 1],
            |row| {
                let source_json: String = row.get(4)?;
                let source = serde_json::from_str::<Option<SourceLocation>>(&source_json)
                    .ok()
                    .flatten();
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    source,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )?;
        let mut candidates = Vec::new();
        let mut truncated = false;
        for row in rows {
            let (node_id, relative_path, kind, label, source, content_hash, size_bytes) = row?;
            let Some(source) = source else {
                continue;
            };
            if candidates.len() == MAX_SEMANTIC_CANDIDATES {
                truncated = true;
                break;
            }
            candidates.push(SemanticSearchCandidate {
                node_id,
                relative_path,
                kind,
                label,
                content_hash,
                size_bytes: size_bytes.max(0) as u64,
                source,
            });
        }
        Ok((candidates, truncated))
    }

    pub(crate) fn search_index_is_available(&self) -> AoneResult<bool> {
        self.connection
            .query_row(
                "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|value| value == Some(1))
            .map_err(Into::into)
    }
}

fn source_candidate_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceSearchCandidate> {
    Ok(SourceSearchCandidate {
        relative_path: row.get(0)?,
        content_hash: row.get(1)?,
        size_bytes: row.get::<_, i64>(2)?.max(0) as u64,
    })
}

fn bounded_collect<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    limit: usize,
) -> AoneResult<(Vec<T>, bool)> {
    let mut values = rows.collect::<Result<Vec<_>, _>>()?;
    let truncated = values.len() > limit;
    values.truncate(limit);
    Ok((values, truncated))
}

fn fts_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) fn rebuild_graph_search(store: &GraphStore) -> AoneResult<()> {
    if !store.search_index_is_available()? {
        return Err(AoneError::InvalidRequest(
            "bundled SQLite was built without the required FTS5 search engine".into(),
        ));
    }
    const KEY: &str = "graphSearchSchema";
    const VERSION: &str = "fts5-trigram-selective-v2";
    let current = store
        .connection
        .query_row(
            "SELECT value FROM projection_metadata WHERE key = ?1",
            params![KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if current.as_deref() == Some(VERSION) {
        return Ok(());
    }
    let transaction = store.connection.unchecked_transaction()?;
    transaction.execute_batch(
        "DROP TRIGGER IF EXISTS graph_nodes_ai;
         DROP TRIGGER IF EXISTS graph_nodes_ad;
         DROP TRIGGER IF EXISTS graph_nodes_au;
         DROP TRIGGER IF EXISTS graph_nodes_au_remove;
         DROP TRIGGER IF EXISTS graph_nodes_au_insert;
         DROP TRIGGER IF EXISTS semantic_search_documents_ad;
         DROP TABLE IF EXISTS graph_search;
         DROP TABLE IF EXISTS semantic_search_documents;",
    )?;
    transaction.execute_batch(super::records::SEMANTIC_SEARCH_SCHEMA)?;
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
    transaction.execute(
        "INSERT INTO projection_metadata (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, VERSION],
    )?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fts_literal;

    #[test]
    fn fts_literals_escape_quotes_without_adding_query_operators() {
        assert_eq!(fts_literal("listen \"ready\""), "\"listen \"\"ready\"\"\"");
    }
}
