use std::time::{Duration, Instant};

pub(super) const MAX_INBOUND_EVENTS_PER_SECOND: u16 = 120;
const WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Admission {
    pub(super) allow_message: bool,
    pub(super) dropped_before: u64,
}

pub(super) struct InboundRateLimiter {
    window_started: Instant,
    emitted: u16,
    dropped: u64,
}

impl InboundRateLimiter {
    pub(super) fn new() -> Self {
        Self::at(Instant::now())
    }

    fn at(now: Instant) -> Self {
        Self {
            window_started: now,
            emitted: 0,
            dropped: 0,
        }
    }

    pub(super) fn admit(&mut self) -> Admission {
        self.admit_at(Instant::now())
    }

    fn admit_at(&mut self, now: Instant) -> Admission {
        let dropped_before = if now
            .checked_duration_since(self.window_started)
            .is_some_and(|elapsed| elapsed >= WINDOW)
        {
            self.window_started = now;
            self.emitted = 0;
            std::mem::take(&mut self.dropped)
        } else {
            0
        };
        if self.emitted < MAX_INBOUND_EVENTS_PER_SECOND {
            self.emitted += 1;
            Admission {
                allow_message: true,
                dropped_before,
            }
        } else {
            self.dropped = self.dropped.saturating_add(1);
            Admission {
                allow_message: false,
                dropped_before,
            }
        }
    }

    pub(super) fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_messages_and_reports_drops_on_next_window() {
        let start = Instant::now();
        let mut limiter = InboundRateLimiter::at(start);
        for _ in 0..MAX_INBOUND_EVENTS_PER_SECOND {
            assert!(limiter.admit_at(start).allow_message);
        }
        assert!(!limiter.admit_at(start).allow_message);
        assert!(!limiter.admit_at(start).allow_message);
        let next = limiter.admit_at(start + WINDOW);
        assert!(next.allow_message);
        assert_eq!(next.dropped_before, 2);
    }

    #[test]
    fn lifecycle_flush_can_take_pending_drops_without_waiting() {
        let start = Instant::now();
        let mut limiter = InboundRateLimiter::at(start);
        for _ in 0..=MAX_INBOUND_EVENTS_PER_SECOND {
            limiter.admit_at(start);
        }
        assert_eq!(limiter.take_dropped(), 1);
        assert_eq!(limiter.take_dropped(), 0);
    }
}
