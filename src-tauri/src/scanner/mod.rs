mod limits;
mod paths;
mod scan;
mod secure_read;
mod time;
mod types;

#[cfg(test)]
mod tests;

pub use limits::{
    MAX_WORKSPACE_FACTS, MAX_WORKSPACE_FILES, MAX_WORKSPACE_SCAN_DURATION,
    MAX_WORKSPACE_SOURCE_BYTES,
};
pub use paths::{
    canonical_workspace, canonicalize_relative_file, is_discoverable_file, is_hard_denied,
    relative_path, workspace_id,
};
pub(crate) use scan::analyze_file_with_deadline;
pub use scan::scan_workspace;
pub use secure_read::read_source_file;
pub(crate) use secure_read::read_source_file_bounded;
pub use time::system_time_string;
pub use types::{IndexedFile, ScanResult};
