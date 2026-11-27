//! Journal intervals: no clock reads or journal locks without a reader.

use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub calls: u64,
    pub elapsed: Duration,
}

/// Nested interval accumulated by name until the next `take`.
/// Names describe engine passes, not document contents.
#[must_use = "the interval ends when its guard is dropped"]
pub struct Span {
    name: &'static str,
    started: Option<Instant>,
    active: Option<u64>,
}

impl Span {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        Self::with_clock(name, Instant::now)
    }

    #[inline]
    fn with_clock(name: &'static str, clock: impl FnOnce() -> Instant) -> Self {
        let started = (super::enabled() && super::kept_here()).then(clock);
        Self {
            name,
            active: started.and_then(|at| super::progress::begin(name, at)),
            started,
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(id) = self.active {
            super::progress::end(id);
        }
        if let Some(started) = self.started {
            // The subscription may have ended before this interval.
            if super::enabled() && super::kept_here() {
                let elapsed = started.elapsed();
                super::with(|j| {
                    let timing = j.timings.entry(self.name).or_default();
                    timing.calls += 1;
                    timing.elapsed += elapsed;
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_span_does_not_read_clock_or_record() {
        let _one = super::super::test_reader().0;
        drop(Span::with_clock("off", || panic!("disabled clock")));
        assert!(super::super::take().timings.is_empty());
    }

    #[test]
    fn nested_spans_aggregate_and_take_resets() {
        let (_one, _reader) = super::super::test_reader();
        let outer = Span::new("outer");
        for _ in 0..2 {
            drop(Span::new("inner"));
        }
        drop(outer);
        let j = super::super::take();
        assert_eq!(j.timings["inner"].calls, 2);
        assert_eq!(j.timings["outer"].calls, 1);
        assert!(j.timings["outer"].elapsed >= j.timings["inner"].elapsed);
        assert!(super::super::take().timings.is_empty());
    }

    #[test]
    fn thread_reader_does_not_time_other_threads() {
        let _one = super::super::test_reader().0;
        let _reader = super::super::subscribe_thread();
        std::thread::spawn(|| {
            drop(Span::with_clock("other", || panic!("unobserved clock")));
        })
        .join()
        .unwrap();
        drop(Span::new("here"));
        let j = super::super::take();
        assert_eq!(j.timings.len(), 1);
        assert_eq!(j.timings["here"].calls, 1);
    }
}
