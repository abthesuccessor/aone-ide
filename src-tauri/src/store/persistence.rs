use std::{
    collections::{BTreeMap, HashMap},
    path::{Component, Path},
};

use rusqlite::{Statement, Transaction, params};
use serde_json::Value;

use super::{
    limits::{GLOBAL_IDENTIFIER_CONFIDENCE, WorkspaceStoreLimits},
    records::{capability_db, evidence_db},
};
use crate::{
    analyzer::stable_id,
    domain::EvidenceKind,
    error::{AoneError, AoneResult},
    scanner::IndexedFile,
};

pub(super) fn validate_indexed_file_limits(
    file: &IndexedFile,
    limits: WorkspaceStoreLimits,
) -> AoneResult<()> {
    if file.file.size_bytes > limits.max_source_bytes {
        return Err(workspace_source_budget_error(limits.max_source_bytes));
    }
    if file.analysis.fact_count > limits.max_facts {
        return Err(workspace_fact_budget_error(limits.max_facts));
    }
    Ok(())
}

pub(super) fn validate_workspace_limits(
    files: &[IndexedFile],
    limits: WorkspaceStoreLimits,
) -> AoneResult<()> {
    if files.len() > limits.max_files {
        return Err(AoneError::InvalidRequest(format!(
            "workspace update exceeded maximum of {} indexed files",
            limits.max_files
        )));
    }
    let mut source_bytes = 0_u64;
    let mut fact_count = 0_usize;
    for file in files {
        validate_indexed_file_limits(file, limits)?;
        source_bytes = source_bytes
            .checked_add(file.file.size_bytes)
            .ok_or_else(|| workspace_source_budget_error(limits.max_source_bytes))?;
        fact_count = fact_count
            .checked_add(file.analysis.fact_count)
            .ok_or_else(|| workspace_fact_budget_error(limits.max_facts))?;
    }
    if source_bytes > limits.max_source_bytes {
        return Err(workspace_source_budget_error(limits.max_source_bytes));
    }
    if fact_count > limits.max_facts {
        return Err(workspace_fact_budget_error(limits.max_facts));
    }
    Ok(())
}

pub(super) fn enforce_workspace_limits(
    transaction: &Transaction<'_>,
    limits: WorkspaceStoreLimits,
) -> AoneResult<()> {
    let (file_count, source_bytes) = transaction.query_row(
        "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM files",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    if file_count < 0 || file_count as u64 > limits.max_files as u64 {
        return Err(AoneError::InvalidRequest(format!(
            "workspace update exceeded maximum of {} indexed files",
            limits.max_files
        )));
    }
    if source_bytes < 0 || source_bytes as u64 > limits.max_source_bytes {
        return Err(workspace_source_budget_error(limits.max_source_bytes));
    }

    let fact_count = transaction.query_row(
        "SELECT COUNT(*) FROM graph_nodes WHERE kind <> 'file'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if fact_count < 0 || fact_count as u64 > limits.max_facts as u64 {
        return Err(workspace_fact_budget_error(limits.max_facts));
    }
    Ok(())
}

fn workspace_source_budget_error(maximum: u64) -> AoneError {
    AoneError::InvalidRequest(format!(
        "workspace update exceeded aggregate source byte budget of {maximum} bytes"
    ))
}

fn workspace_fact_budget_error(maximum: usize) -> AoneError {
    AoneError::InvalidRequest(format!(
        "workspace update exceeded aggregate extracted fact budget of {maximum} facts"
    ))
}

pub(super) fn insert_indexed_file(
    transaction: &Transaction<'_>,
    indexed: &IndexedFile,
) -> AoneResult<()> {
    let file = &indexed.file;
    transaction.execute(
        "INSERT INTO files
         (id, relative_path, language, capability, size_bytes, content_hash, modified_at, parse_errors)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            file.id,
            file.relative_path,
            file.language,
            capability_db(file.capability),
            file.size_bytes as i64,
            file.content_hash,
            file.modified_at,
            file.parse_errors,
        ],
    )?;
    transaction.execute(
        "INSERT INTO search_documents (file_path) VALUES (?1)",
        params![file.relative_path],
    )?;
    let search_row_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO source_search (rowid, content) VALUES (?1, ?2)",
        params![search_row_id, indexed.source],
    )?;
    for node in &indexed.analysis.nodes {
        transaction.execute(
            "INSERT INTO graph_nodes
             (id, file_path, kind, label, source_json, language, evidence, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                node.id,
                file.relative_path,
                node.kind,
                node.label,
                serde_json::to_string(&node.source)?,
                node.language,
                evidence_db(node.evidence),
                serde_json::to_string(&node.metadata)?,
            ],
        )?;
    }
    for edge in &indexed.analysis.edges {
        transaction.execute(
            "INSERT INTO graph_edges
             (id, file_path, source, target, kind, evidence, confidence, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                edge.id,
                file.relative_path,
                edge.source,
                edge.target,
                edge.kind,
                evidence_db(edge.evidence),
                edge.confidence,
                serde_json::to_string(&edge.metadata)?,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn rebuild_resolution_edges(transaction: &Transaction<'_>) -> AoneResult<()> {
    // `resolvesTo` edges are projections derived from the current set of
    // indexed files. Delete both resolved and inferred projections so a
    // newly ambiguous identifier or a removed file cannot leave a stale
    // relationship behind.
    transaction.execute("DELETE FROM graph_edges WHERE kind = 'resolvesTo'", [])?;

    let declarations = {
        let mut statement = transaction.prepare(
            "SELECT id, label FROM graph_nodes
             WHERE kind IN ('function', 'method', 'class', 'service', 'repository', 'model', 'controller')",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut declarations_by_name = HashMap::<String, Vec<String>>::new();
    for (id, label) in declarations {
        declarations_by_name
            .entry(label.trim().to_ascii_lowercase())
            .or_default()
            .push(id);
    }

    let call_targets = {
        let mut statement = transaction
            .prepare("SELECT id, file_path, label FROM graph_nodes WHERE kind = 'callTarget'")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut insert_statement = transaction.prepare(
        "INSERT INTO graph_edges
         (id, file_path, source, target, kind, evidence, confidence, metadata_json)
         VALUES (?1, ?2, ?3, ?4, 'resolvesTo', ?5, ?6, ?7)",
    )?;
    for (source, file_path, label) in call_targets {
        let Some(identifier) = final_call_identifier(&label) else {
            continue;
        };
        let Some(targets) = declarations_by_name.get(&identifier.to_ascii_lowercase()) else {
            continue;
        };
        if targets.len() == 1 {
            insert_resolution_edge(
                &mut insert_statement,
                &file_path,
                &source,
                &targets[0],
                EvidenceKind::Inferred,
                Some(GLOBAL_IDENTIFIER_CONFIDENCE),
                BTreeMap::from([
                    (
                        "resolver".to_string(),
                        Value::String("globalExactFinalIdentifier".into()),
                    ),
                    (
                        "matchScope".to_string(),
                        Value::String("workspaceGlobal".into()),
                    ),
                    (
                        "basis".to_string(),
                        Value::String(
                            "unique final callee identifier matched one declaration label; lexical scope and import binding were not proven"
                                .into(),
                        ),
                    ),
                ]),
            )?;
        }
    }

    resolve_relative_imports(transaction, &mut insert_statement)?;
    Ok(())
}

fn resolve_relative_imports(
    transaction: &Transaction<'_>,
    insert_statement: &mut Statement<'_>,
) -> AoneResult<()> {
    let files = {
        let mut statement =
            transaction.prepare("SELECT file_path, id FROM graph_nodes WHERE kind = 'file'")?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?
    };
    let imports = {
        let mut statement = transaction
            .prepare("SELECT id, file_path, label FROM graph_nodes WHERE kind = 'module'")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (source, source_file, label) in imports {
        let Some(specifier) = quoted_module_specifier(&label) else {
            continue;
        };
        if !specifier.starts_with('.') {
            continue;
        }
        let Some(base) = normalize_import_path(&source_file, specifier) else {
            continue;
        };
        let mut candidates = Vec::new();
        if let Some(id) = files.get(&base) {
            candidates.push(id.clone());
        }
        for extension in ["ts", "tsx", "js", "jsx", "mts", "cts"] {
            if let Some(id) = files.get(&format!("{base}.{extension}")) {
                candidates.push(id.clone());
            }
            if let Some(id) = files.get(&format!("{base}/index.{extension}")) {
                candidates.push(id.clone());
            }
        }
        candidates.sort();
        candidates.dedup();
        if candidates.len() == 1 {
            insert_resolution_edge(
                insert_statement,
                &source_file,
                &source,
                &candidates[0],
                EvidenceKind::Resolved,
                None,
                BTreeMap::from([
                    (
                        "resolver".to_string(),
                        Value::String("relativeImportPath".into()),
                    ),
                    (
                        "basis".to_string(),
                        Value::String(
                            "normalized relative module specifier uniquely matched an indexed file"
                                .into(),
                        ),
                    ),
                ]),
            )?;
        }
    }
    Ok(())
}

fn insert_resolution_edge(
    statement: &mut Statement<'_>,
    file_path: &str,
    source: &str,
    target: &str,
    evidence: EvidenceKind,
    confidence: Option<f64>,
    metadata: BTreeMap<String, Value>,
) -> AoneResult<()> {
    let id = stable_id("edge", &[source, target, "resolvesTo"]);
    let metadata = serde_json::to_string(&metadata)?;
    statement.execute(params![
        id,
        file_path,
        source,
        target,
        evidence_db(evidence),
        confidence,
        metadata
    ])?;
    Ok(())
}

fn final_call_identifier(label: &str) -> Option<&str> {
    let without_arguments = label.split('(').next()?.trim();
    let identifier = without_arguments
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())?
        .trim_matches(|character: char| !character.is_alphanumeric() && character != '_');
    (!identifier.is_empty()).then_some(identifier)
}

fn quoted_module_specifier(label: &str) -> Option<&str> {
    let mut result = None;
    for quote in ['\'', '"'] {
        let mut positions = label.match_indices(quote).map(|(index, _)| index);
        while let (Some(start), Some(end)) = (positions.next(), positions.next()) {
            if end > start + 1 {
                result = Some(&label[start + 1..end]);
            }
        }
    }
    result
}

fn normalize_import_path(source_file: &str, specifier: &str) -> Option<String> {
    let mut parts = Path::new(source_file)
        .parent()?
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in Path::new(specifier).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(value) => parts.push(value.to_str()?.to_owned()),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}
