//! Длина серии кадров: фиксированное число или по длительности.
//!
//! Без `series_ms` серия ровно `runs` кадров (прежнее поведение скриптов).
//! С ним серия идёт, пока не набрано и `min_runs` кадров, и `series_ms`
//! миллисекунд от начала серии, но не длиннее `runs`: лёгкая страница даёт много
//! образцов за то же время, тяжёлая — не меньше `min_runs`.

use std::time::Duration;

#[derive(Clone, Copy)]
pub(super) struct Series {
    max: usize,
    min: usize,
    budget: Duration,
}

impl Series {
    pub(super) fn new(args: &super::args::Args) -> Self {
        Self::with(args.runs, args.min_runs, args.series_ms)
    }

    pub(super) fn with(runs: usize, min_runs: usize, series_ms: u64) -> Self {
        if series_ms == 0 {
            return Self { max: runs, min: runs, budget: Duration::ZERO };
        }
        Self { max: runs, min: min_runs.clamp(1, runs.max(1)), budget: Duration::from_millis(series_ms) }
    }

    /// Ещё один кадр после `done` кадров, занявших `spent`?
    pub(super) fn more(&self, done: usize, spent: Duration) -> bool {
        done < self.max && (done < self.min || spent < self.budget)
    }

    /// Наименьшая допустимая длина серии.
    pub(super) fn min_frames(&self) -> usize {
        self.min
    }

    /// Серия по длительности (а не фиксированная): её длина известна только в конце.
    pub(super) fn timed(&self) -> bool {
        !self.budget.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_series_runs_exactly_the_requested_frames() {
        let s = Series::with(6, 3, 0);
        assert!(s.more(5, Duration::from_secs(9)));
        assert!(!s.more(6, Duration::ZERO));
        assert!(!s.timed());
    }

    #[test]
    fn timed_series_needs_both_the_minimum_and_the_duration_but_stops_at_the_maximum() {
        let s = Series::with(30, 3, 500);
        assert!(s.more(2, Duration::from_secs(5)));
        assert!(s.more(10, Duration::from_millis(499)));
        assert!(!s.more(10, Duration::from_millis(500)));
        assert!(!s.more(30, Duration::ZERO));
        assert!(Series::with(2, 3, 500).more(1, Duration::from_secs(1)));
        assert!(!Series::with(2, 3, 500).more(2, Duration::ZERO));
    }
}
