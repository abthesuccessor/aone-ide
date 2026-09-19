use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use ignore::WalkBuilder;

use crate::{
    analyzer::{analyze_source_with_deadline, language_support, stable_id},
    domain::{ScanProgress, WorkspaceFile},
    error::{AoneError, AoneResult},
};

use super::{
    IndexedFile, ScanResult, canonical_workspace,
    limits::{
        DEFAULT_SCAN_LIMITS, MAX_INDEX_FILE_BYTES, ScanLimits, enforce_time_budget,
        workspace_fact_budget_error, workspace_source_budget_error,
    },
    paths::is_hard_denied,
    relative_path,
    secure_read::{is_binary, open_verified_regular_file, read_bounded},
    system_time_string,
};

pub fn scan_workspace<F>(root: &Path, workspace_id: &str, mut progress: F) -> AoneResult<ScanResult>
where
    F: FnMut(ScanProgress),
{
    scan_workspace_with_limits(root, workspace_id, &mut progress, DEFAULT_SCAN_LIMITS)
}

pub(super) fn scan_workspace_with_limits<F>(
    root: &Path,
    workspace_id: &str,
    progress: &mut F,
    limits: ScanLimits,
) -> AoneResult<ScanResult>
where
    F: FnMut(ScanProgress),
{
    let started = Instant::now();
    let canonical_root = canonical_workspace(root)?;
    let root = canonical_root.as_path();
    let paths = discover_files(root, started, limits)?;
    let deadline = started.checked_add(limits.max_duration).ok_or_else(|| {
        AoneError::InvalidRequest("workspace scan duration is outside the supported range".into())
    })?;
    let total = paths.len();
    progress(ScanProgress {
        phase: "analyzing".into(),
        completed: 0,
        total,
        current_path: None,
    });

    let mut files = Vec::with_capacity(total);
    let mut aggregate_source_bytes = 0_u64;
    let mut aggregate_facts = 0_usize;
    for (index, path) in paths.into_iter().enumerate() {
        enforce_time_budget(started, limits.max_duration, "analysis")?;
        let relative = relative_path(root, &path)?;
        progress(ScanProgress {
            phase: "analyzing".into(),
            completed: index,
            total,
            current_path: Some(relative.clone()),
        });
        if let Some(file) = analyze_file_with_deadline(root, workspace_id, &path, Some(deadline))? {
            aggregate_source_bytes = aggregate_source_bytes
                .checked_add(file.file.size_bytes)
                .ok_or_else(|| workspace_source_budget_error(limits.max_source_bytes))?;
            if aggregate_source_bytes > limits.max_source_bytes {
                return Err(workspace_source_budget_error(limits.max_source_bytes));
            }
            aggregate_facts = aggregate_facts
                .checked_add(file.analysis.fact_count)
                .ok_or_else(|| workspace_fact_budget_error(limits.max_facts))?;
            if aggregate_facts > limits.max_facts {
                return Err(workspace_fact_budget_error(limits.max_facts));
            }
            files.push(file);
        }
        enforce_time_budget(started, limits.max_duration, "analysis")?;
    }

    progress(ScanProgress {
        // Source analysis is complete, but callers may still need to persist
        // and install the result. Reserve `complete` for that commit boundary.
        phase: "analyzed".into(),
        completed: total,
        total,
        current_path: None,
    });
    Ok(ScanResult { files })
}

#[cfg(test)]
pub(super) fn analyze_file(
    root: &Path,
    workspace_id: &str,
    path: &Path,
) -> AoneResult<Option<IndexedFile>> {
    analyze_file_with_deadline(root, workspace_id, path, None)
}

pub(crate) fn analyze_file_with_deadline(
    root: &Path,
    workspace_id: &str,
    path: &Path,
    deadline: Option<Instant>,
) -> AoneResult<Option<IndexedFile>> {
    let path_metadata = path.symlink_metadata()?;
    if path_metadata.file_type().is_symlink() {
        return Err(AoneError::SensitivePath);
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(AoneError::PathEscape);
    }
    let relative = relative_path(root, &canonical)?;
    let canonical_metadata = canonical.metadata()?;
    let (file, opened_metadata) =
        open_verified_regular_file(path, &path_metadata, &canonical_metadata)?;
    if opened_metadata.len() > MAX_INDEX_FILE_BYTES {
        return Ok(None);
    }

    let (bytes, metadata) = read_bounded(file, &opened_metadata, path, MAX_INDEX_FILE_BYTES)?;
    if is_binary(&bytes) {
        return Ok(None);
    }
    let size_bytes = bytes.len() as u64;
    let Ok(source) = String::from_utf8(bytes) else {
        return Ok(None);
    };

    let content_hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let support = language_support(Path::new(&relative));
    let analysis =
        analyze_source_with_deadline(workspace_id, &relative, &source, &content_hash, deadline)?;
    let modified_at = metadata
        .modified()
        .ok()
        .map(system_time_string)
        .unwrap_or_else(|| "unknown".into());
    let file = WorkspaceFile {
        id: stable_id("file", &[workspace_id, &relative]),
        relative_path: relative,
        language: support.name.into(),
        capability: support.capability,
        size_bytes,
        content_hash,
        modified_at,
        parse_errors: analysis.parse_errors,
    };
    Ok(Some(IndexedFile {
        file,
        source,
        analysis,
    }))
}

fn discover_files(root: &Path, started: Instant, limits: ScanLimits) -> AoneResult<Vec<PathBuf>> {
    let root_for_filter = root.to_path_buf();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .parents(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.path() == root_for_filter
                || entry
                    .path()
                    .strip_prefix(&root_for_filter)
                    .is_ok_and(|relative| !is_hard_denied(relative))
        });

    let mut paths = Vec::with_capacity(limits.max_files.min(1_024));
    let mut aggregate_source_bytes = 0_u64;
    for entry in builder.build().filter_map(Result::ok) {
        enforce_time_budget(started, limits.max_duration, "discovery")?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > MAX_INDEX_FILE_BYTES {
            continue;
        }
        if paths.len() >= limits.max_files {
            return Err(AoneError::InvalidRequest(format!(
                "workspace scan exceeded maximum of {} qualifying files",
                limits.max_files
            )));
        }
        aggregate_source_bytes = aggregate_source_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| workspace_source_budget_error(limits.max_source_bytes))?;
        if aggregate_source_bytes > limits.max_source_bytes {
            return Err(workspace_source_budget_error(limits.max_source_bytes));
        }
        paths.push(entry.into_path());
    }
    paths.sort_unstable();
    Ok(paths)
}
