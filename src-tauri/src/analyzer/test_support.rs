use crate::error::AoneResult;

use super::{AnalyzedSource, extract::analyze_source_with_deadline};

pub fn analyze_source(
    workspace_id: &str,
    relative_path: &str,
    source: &str,
    content_hash: &str,
) -> AoneResult<AnalyzedSource> {
    analyze_source_with_deadline(workspace_id, relative_path, source, content_hash, None)
}
