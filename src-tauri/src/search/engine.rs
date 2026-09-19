use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

use crate::{
    domain::{WorkspaceSearchMatch, WorkspaceSearchMatchKind, WorkspaceSearchResult},
    error::AoneError,
    scanner::{canonicalize_relative_file, read_source_file_bounded},
    store::{SearchCandidates, SemanticSearchCandidate, SourceSearchCandidate},
};

use super::{
    limits::{
        MAX_MATCHES_PER_FILE, MAX_RESULTS, MAX_SOURCE_READ_ATTEMPTS, MAX_SOURCE_READ_BYTES,
        SEARCH_TIME_BUDGET,
    },
    matching::{
        bounded_match_text, first_literal_range, literal_line_matches, match_key, path_match,
        preview_line,
    },
    state::SearchToken,
};

struct VerifiedSource {
    source: String,
}

struct SearchBudget {
    started: Instant,
    bytes_charged: usize,
    read_attempts: usize,
    observed_matches: usize,
    truncated: bool,
    stopped: bool,
    token: SearchToken,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchExecutionStats {
    pub(super) bytes_charged: usize,
    pub(super) read_attempts: usize,
}

pub(super) fn execute_search(
    root: &Path,
    query: &str,
    requested_limit: usize,
    candidates: SearchCandidates,
    token: SearchToken,
) -> WorkspaceSearchResult {
    let (result, stats) = execute_search_inner(root, query, requested_limit, candidates, token);
    debug_assert!(stats.bytes_charged <= MAX_SOURCE_READ_BYTES);
    debug_assert!(stats.read_attempts <= MAX_SOURCE_READ_ATTEMPTS);
    result
}

fn execute_search_inner(
    root: &Path,
    query: &str,
    requested_limit: usize,
    candidates: SearchCandidates,
    token: SearchToken,
) -> (WorkspaceSearchResult, SearchExecutionStats) {
    let limit = requested_limit.min(MAX_RESULTS);
    let indexed_file_count = candidates.indexed_file_count;
    let mut budget = SearchBudget {
        started: Instant::now(),
        bytes_charged: 0,
        read_attempts: 0,
        observed_matches: 0,
        truncated: candidates.truncated,
        stopped: false,
        token,
    };
    let mut cache = HashMap::<String, Option<VerifiedSource>>::new();
    let mut matches = Vec::with_capacity(limit.saturating_add(1).min(128));
    let mut seen = HashSet::new();

    add_semantic_matches(
        root,
        &candidates.semantic_nodes,
        query,
        &mut cache,
        &mut budget,
        &mut matches,
        &mut seen,
        limit,
    );
    if matches.len() <= limit && !budget.should_stop() {
        add_path_matches(
            &candidates.path_files,
            query,
            &mut budget,
            &mut matches,
            &mut seen,
            limit,
        );
    }
    if matches.len() <= limit && !budget.should_stop() {
        add_content_matches(
            root,
            &candidates.source_files,
            query,
            &mut cache,
            &mut budget,
            &mut matches,
            &mut seen,
            limit,
        );
    }

    matches.sort_by(|left, right| {
        kind_rank(left.kind)
            .cmp(&kind_rank(right.kind))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.start_column.cmp(&right.start_column))
            .then_with(|| left.key.cmp(&right.key))
    });
    let overflowed = matches.len() > limit;
    let total_matches = budget.observed_matches;
    matches.truncate(limit);
    let result = WorkspaceSearchResult {
        query: query.into(),
        matches,
        total_matches,
        truncated: budget.truncated || overflowed,
        indexed_file_count,
    };
    let stats = SearchExecutionStats {
        bytes_charged: budget.bytes_charged,
        read_attempts: budget.read_attempts,
    };
    (result, stats)
}

fn add_path_matches(
    candidates: &[SourceSearchCandidate],
    query: &str,
    budget: &mut SearchBudget,
    matches: &mut Vec<WorkspaceSearchMatch>,
    seen: &mut HashSet<String>,
    limit: usize,
) {
    for candidate in candidates {
        let Some(found) = path_match(&candidate.relative_path, query) else {
            continue;
        };
        budget.observed_matches += 1;
        push_unique(matches, seen, found);
        if matches.len() > limit {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_semantic_matches(
    root: &Path,
    candidates: &[SemanticSearchCandidate],
    query: &str,
    cache: &mut HashMap<String, Option<VerifiedSource>>,
    budget: &mut SearchBudget,
    matches: &mut Vec<WorkspaceSearchMatch>,
    seen: &mut HashSet<String>,
    limit: usize,
) {
    let mut exact = candidates
        .iter()
        .filter_map(|candidate| {
            first_literal_range(&candidate.label, query).map(|range| (candidate, range))
        })
        .collect::<Vec<_>>();
    exact.sort_by_key(|(candidate, _)| semantic_rank(&candidate.kind));
    for (candidate, (match_start, match_end)) in exact {
        if budget.should_stop() {
            return;
        }
        let Some(source) = verified_source(
            root,
            &candidate.relative_path,
            &candidate.content_hash,
            candidate.size_bytes,
            cache,
            budget,
        ) else {
            continue;
        };
        let kind = semantic_kind(&candidate.kind);
        let location = &candidate.source;
        let found = WorkspaceSearchMatch {
            key: format!(
                "{}:{}",
                match_key(
                    kind,
                    &candidate.relative_path,
                    location.start_line,
                    location.start_column,
                    0
                ),
                candidate.node_id
            ),
            relative_path: candidate.relative_path.clone(),
            start_line: location.start_line,
            start_column: location.start_column,
            end_line: location.end_line,
            end_column: location.end_column,
            preview: preview_line(&source.source, location.start_line),
            match_text: bounded_match_text(&candidate.label[match_start..match_end]),
            kind,
        };
        budget.observed_matches += 1;
        push_unique(matches, seen, found);
        if matches.len() > limit {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_content_matches(
    root: &Path,
    candidates: &[SourceSearchCandidate],
    query: &str,
    cache: &mut HashMap<String, Option<VerifiedSource>>,
    budget: &mut SearchBudget,
    matches: &mut Vec<WorkspaceSearchMatch>,
    seen: &mut HashSet<String>,
    limit: usize,
) {
    for candidate in candidates {
        if budget.should_stop() {
            return;
        }
        let Some(source) = verified_source(
            root,
            &candidate.relative_path,
            &candidate.content_hash,
            candidate.size_bytes,
            cache,
            budget,
        ) else {
            continue;
        };
        let mut found = literal_line_matches(
            &candidate.relative_path,
            &source.source,
            query,
            MAX_MATCHES_PER_FILE + 1,
        );
        budget.observed_matches = budget.observed_matches.saturating_add(found.len());
        if found.len() > MAX_MATCHES_PER_FILE {
            budget.truncated = true;
            found.truncate(MAX_MATCHES_PER_FILE);
        }
        for item in found {
            push_unique(matches, seen, item);
            if matches.len() > limit {
                return;
            }
        }
    }
}

fn verified_source<'a>(
    root: &Path,
    relative_path: &str,
    expected_hash: &str,
    indexed_size: u64,
    cache: &'a mut HashMap<String, Option<VerifiedSource>>,
    budget: &mut SearchBudget,
) -> Option<&'a VerifiedSource> {
    if !cache.contains_key(relative_path) {
        let Some(read_limit) = budget.reserve_read(indexed_size) else {
            cache.insert(relative_path.into(), None);
            return None;
        };
        let loaded = canonicalize_relative_file(root, Path::new(relative_path))
            .and_then(|path| read_source_file_bounded(&path, read_limit));
        match loaded {
            Ok(source) => {
                if !budget.reconcile_actual(source.len(), indexed_size) {
                    cache.insert(relative_path.into(), None);
                    return None;
                }
                if blake3::hash(source.as_bytes()).to_hex().as_str() == expected_hash {
                    cache.insert(relative_path.into(), Some(VerifiedSource { source }));
                } else {
                    budget.truncated = true;
                    cache.insert(relative_path.into(), None);
                }
            }
            Err(error) => {
                budget.truncated = true;
                if !is_pre_open_missing(&error) {
                    budget.exhaust();
                }
                cache.insert(relative_path.into(), None);
            }
        }
    }
    cache.get(relative_path).and_then(Option::as_ref)
}

fn is_pre_open_missing(error: &AoneError) -> bool {
    matches!(error, AoneError::Io(source) if source.kind() == std::io::ErrorKind::NotFound)
}

impl SearchBudget {
    fn should_stop(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if !self.token.is_current()
            || self.started.elapsed() >= SEARCH_TIME_BUDGET
            || self.bytes_charged >= MAX_SOURCE_READ_BYTES
            || self.read_attempts >= MAX_SOURCE_READ_ATTEMPTS
        {
            self.truncated = true;
            self.stopped = true;
        }
        self.stopped
    }

    fn reserve_read(&mut self, indexed_size: u64) -> Option<u64> {
        if self.should_stop() {
            return None;
        }
        let indexed_size = match usize::try_from(indexed_size) {
            Ok(value) => value,
            Err(_) => {
                self.exhaust();
                return None;
            }
        };
        let remaining = MAX_SOURCE_READ_BYTES.saturating_sub(self.bytes_charged);
        if indexed_size > remaining {
            self.exhaust();
            return None;
        }
        self.bytes_charged += indexed_size;
        self.read_attempts += 1;
        Some(remaining as u64)
    }

    fn reconcile_actual(&mut self, actual_size: usize, indexed_size: u64) -> bool {
        if actual_size > MAX_SOURCE_READ_BYTES {
            self.exhaust();
            return false;
        }
        let indexed_size = usize::try_from(indexed_size).unwrap_or(usize::MAX);
        let remaining = MAX_SOURCE_READ_BYTES.saturating_sub(self.bytes_charged);
        let extra = actual_size.saturating_sub(indexed_size);
        if extra > remaining {
            self.exhaust();
            return false;
        }
        self.bytes_charged += extra;
        true
    }

    fn exhaust(&mut self) {
        self.bytes_charged = MAX_SOURCE_READ_BYTES;
        self.truncated = true;
        self.stopped = true;
    }
}

fn push_unique(
    matches: &mut Vec<WorkspaceSearchMatch>,
    seen: &mut HashSet<String>,
    found: WorkspaceSearchMatch,
) {
    if seen.insert(found.key.clone()) {
        matches.push(found);
    }
}

fn semantic_kind(kind: &str) -> WorkspaceSearchMatchKind {
    match kind.to_ascii_lowercase().as_str() {
        "endpoint" | "api" | "route" => WorkspaceSearchMatchKind::Endpoint,
        "event" | "listener" | "subscriber" | "publisher" => WorkspaceSearchMatchKind::Event,
        "heading" => WorkspaceSearchMatchKind::Heading,
        "sentence" => WorkspaceSearchMatchKind::Sentence,
        _ => WorkspaceSearchMatchKind::Symbol,
    }
}

fn semantic_rank(kind: &str) -> u8 {
    kind_rank(semantic_kind(kind))
}

fn kind_rank(kind: WorkspaceSearchMatchKind) -> u8 {
    match kind {
        WorkspaceSearchMatchKind::Endpoint => 0,
        WorkspaceSearchMatchKind::Event => 1,
        WorkspaceSearchMatchKind::Heading => 2,
        WorkspaceSearchMatchKind::Sentence => 3,
        WorkspaceSearchMatchKind::Symbol => 4,
        WorkspaceSearchMatchKind::Path => 5,
        WorkspaceSearchMatchKind::Content => 6,
    }
}

#[cfg(test)]
pub(super) fn execute_search_with_stats(
    root: &Path,
    query: &str,
    requested_limit: usize,
    candidates: SearchCandidates,
    token: SearchToken,
) -> (WorkspaceSearchResult, SearchExecutionStats) {
    execute_search_inner(root, query, requested_limit, candidates, token)
}
