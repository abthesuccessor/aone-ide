use crate::scanner::{MAX_WORKSPACE_FACTS, MAX_WORKSPACE_FILES, MAX_WORKSPACE_SOURCE_BYTES};

pub(super) const MAX_GRAPH_NODES: usize = 500;
pub(super) const MAX_GRAPH_DEPTH: usize = 4;
pub(super) const MAX_GRAPH_ROOTS: usize = 50;
pub(super) const MAX_SEED_NODES: usize = 100;
pub(super) const GRAPH_EDGE_MULTIPLIER: usize = 4;
pub(super) const GLOBAL_IDENTIFIER_CONFIDENCE: f64 = 0.6;

#[derive(Clone, Copy)]
pub(super) struct WorkspaceStoreLimits {
    pub(super) max_files: usize,
    pub(super) max_source_bytes: u64,
    pub(super) max_facts: usize,
}

pub(super) const DEFAULT_WORKSPACE_STORE_LIMITS: WorkspaceStoreLimits = WorkspaceStoreLimits {
    max_files: MAX_WORKSPACE_FILES,
    max_source_bytes: MAX_WORKSPACE_SOURCE_BYTES,
    max_facts: MAX_WORKSPACE_FACTS,
};
