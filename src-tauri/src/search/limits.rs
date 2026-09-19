use std::time::Duration;

pub(super) const MAX_QUERY_CHARS: usize = 256;
pub(super) const MAX_RESULTS: usize = 500;
pub(super) const MAX_MATCHES_PER_FILE: usize = 50;
pub(super) const MAX_PREVIEW_CHARS: usize = 320;
pub(super) const MAX_MATCH_TEXT_CHARS: usize = 256;
pub(super) const MAX_SOURCE_READ_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_SOURCE_READ_ATTEMPTS: usize = 2_000;
pub(super) const SEARCH_TIME_BUDGET: Duration = Duration::from_millis(750);
