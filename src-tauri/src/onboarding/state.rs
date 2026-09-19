use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::error::{AoneError, AoneResult};

pub struct OnboardingState {
    active: Arc<AtomicBool>,
}

pub(super) struct OnboardingReservation {
    active: Arc<AtomicBool>,
}

impl OnboardingState {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn reserve(&self) -> AoneResult<OnboardingReservation> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                AoneError::InvalidRequest(
                    "another project onboarding operation is already in progress".into(),
                )
            })?;
        Ok(OnboardingReservation {
            active: Arc::clone(&self.active),
        })
    }
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OnboardingReservation {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}
