use std::time::{Duration, Instant};

use crate::error::{AoneError, AoneResult};

pub const MAX_INDEX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_PREVIEW_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_WORKSPACE_FILES: usize = 20_000;
pub const MAX_WORKSPACE_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_WORKSPACE_FACTS: usize = 500_000;
pub const MAX_WORKSPACE_SCAN_DURATION: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy)]
pub(super) struct ScanLimits {
    pub(super) max_files: usize,
    pub(super) max_source_bytes: u64,
    pub(super) max_facts: usize,
    pub(super) max_duration: Duration,
}

pub(super) const DEFAULT_SCAN_LIMITS: ScanLimits = ScanLimits {
    max_files: MAX_WORKSPACE_FILES,
    max_source_bytes: MAX_WORKSPACE_SOURCE_BYTES,
    max_facts: MAX_WORKSPACE_FACTS,
    max_duration: MAX_WORKSPACE_SCAN_DURATION,
};

pub(super) fn enforce_time_budget(
    started: Instant,
    maximum: Duration,
    phase: &str,
) -> AoneResult<()> {
    if started.elapsed() >= maximum {
        return Err(AoneError::InvalidRequest(format!(
            "workspace scan exceeded wall-clock budget of {} seconds during {phase}",
            maximum.as_secs()
        )));
    }
    Ok(())
}

pub(super) fn workspace_source_budget_error(maximum: u64) -> AoneError {
    AoneError::InvalidRequest(format!(
        "workspace scan exceeded aggregate source byte budget of {maximum} bytes"
    ))
}

pub(super) fn workspace_fact_budget_error(maximum: usize) -> AoneError {
    AoneError::InvalidRequest(format!(
        "workspace scan exceeded aggregate extracted fact budget of {maximum} facts"
    ))
}
