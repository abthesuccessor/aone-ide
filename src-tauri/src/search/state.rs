use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub struct SearchState {
    latest_generation: Arc<AtomicU64>,
}

#[derive(Clone)]
pub(super) struct SearchToken {
    latest_generation: Arc<AtomicU64>,
    generation: u64,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            latest_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn begin(&self) -> SearchToken {
        let generation = self
            .latest_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        SearchToken {
            latest_generation: Arc::clone(&self.latest_generation),
            generation,
        }
    }
}

impl SearchToken {
    pub(super) fn is_current(&self) -> bool {
        self.latest_generation.load(Ordering::Acquire) == self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::SearchState;

    #[test]
    fn beginning_a_search_cancels_the_previous_token() {
        let state = SearchState::new();
        let first = state.begin();
        assert!(first.is_current());
        let second = state.begin();
        assert!(!first.is_current());
        assert!(second.is_current());
    }
}
