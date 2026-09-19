use std::path::PathBuf;

use super::RuntimeState;

impl Default for RuntimeState {
    fn default() -> Self {
        Self::new(PathBuf::new())
    }
}
