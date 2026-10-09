//! Период сводок и ограниченный idle heartbeat; persisted tree state не активность.

use std::time::{Duration, Instant};

pub(super) struct Clock {
    sample: Instant,
    emitted: Instant,
}

impl Clock {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            sample: now,
            emitted: now,
        }
    }

    pub(super) fn sample(&mut self, now: Instant) -> Option<Duration> {
        let interval = now.duration_since(self.sample);
        if interval < Duration::from_secs(1) {
            return None;
        }
        self.sample = now;
        Some(interval)
    }

    pub(super) fn should_emit(&mut self, active: bool, now: Instant) -> bool {
        if !active && now.duration_since(self.emitted) < Duration::from_secs(60) {
            return false;
        }
        self.emitted = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_is_bounded_active_is_immediate_and_interval_is_actual() {
        let now = Instant::now();
        let mut clock = Clock::new(now);
        assert_eq!(clock.sample(now + Duration::from_millis(999)), None);
        assert_eq!(
            clock.sample(now + Duration::from_millis(1200)),
            Some(Duration::from_millis(1200))
        );
        for sec in 1..60 {
            assert!(!clock.should_emit(false, now + Duration::from_secs(sec)));
        }
        assert!(clock.should_emit(false, now + Duration::from_secs(60)));
        assert!(clock.should_emit(true, now + Duration::from_secs(61)));
        assert!(!clock.should_emit(false, now + Duration::from_secs(62)));
        assert!(clock.should_emit(false, now + Duration::from_secs(121)));
    }
}
