//! KaminIDE patch: application-owned startup intervals; no clock reads when disabled.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Time includes nested intervals; entries cannot be added as disjoint phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartupTiming {
    pub calls: u64,
    pub elapsed: Duration,
}

type Timings = Arc<Mutex<BTreeMap<&'static str, StartupTiming>>>;

#[derive(Clone, Default)]
pub struct StartupProfile {
    timings: Option<Timings>,
}

impl StartupProfile {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            timings: enabled.then(Arc::default),
        }
    }

    /// Create the guard before the operation, including platform construction.
    #[inline]
    pub fn span(&self, name: &'static str) -> StartupSpan {
        self.with_clock(name, Instant::now)
    }

    #[inline]
    fn with_clock(&self, name: &'static str, clock: impl FnOnce() -> Instant) -> StartupSpan {
        StartupSpan {
            name,
            open: self
                .timings
                .as_ref()
                .map(|timings| (timings.clone(), clock())),
        }
    }

    /// Drain intervals without changing configuration or reading the clock.
    pub fn take(&self) -> BTreeMap<&'static str, StartupTiming> {
        self.timings
            .as_ref()
            .and_then(|timings| timings.lock().ok())
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default()
    }
}

#[must_use = "the interval ends when its guard is dropped"]
pub struct StartupSpan {
    name: &'static str,
    open: Option<(Timings, Instant)>,
}

impl Drop for StartupSpan {
    fn drop(&mut self) {
        if let Some((profile, started)) = &self.open {
            let elapsed = started.elapsed();
            if let Ok(mut timings) = profile.lock() {
                let timing = timings.entry(self.name).or_default();
                timing.calls += 1;
                timing.elapsed += elapsed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_does_not_read_clock_or_retain_profile() {
        let profile = StartupProfile::new(false);
        let span = profile.with_clock("off", || panic!("disabled clock"));
        assert!(profile.timings.is_none());
        drop(span);
        assert!(profile.take().is_empty());
    }

    #[test]
    fn clones_share_counts_and_take_resets() {
        let profile = StartupProfile::new(true);
        let clone = profile.clone();
        let outer = profile.span("platform");
        drop(clone.span("fonts"));
        drop(profile.span("fonts"));
        drop(outer);
        let timings = clone.take();
        assert_eq!(timings["fonts"].calls, 2);
        assert_eq!(timings["platform"].calls, 1);
        assert!(timings["platform"].elapsed >= timings["fonts"].elapsed);
        assert!(profile.take().is_empty());
        assert!(StartupProfile::new(true).take().is_empty());
    }
}
