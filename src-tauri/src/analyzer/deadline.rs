use std::time::Instant;

use crate::error::{AoneError, AoneResult};

pub(super) fn enforce_analysis_deadline(deadline: Option<Instant>) -> AoneResult<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(AoneError::InvalidRequest(
            "workspace scan exceeded its wall-clock budget during source analysis".into(),
        ));
    }
    Ok(())
}
