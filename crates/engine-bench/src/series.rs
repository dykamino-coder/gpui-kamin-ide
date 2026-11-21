//! Length of a frame series: a fixed count or adaptive by time and stability.
//!
//! Without `series_ms` a series is exactly `runs` frames (the scripts' old behaviour).
//! With it the series runs `min_runs` frames, then while `series_ms` from its
//! start have not passed and the frames are not yet stable, never longer than
//! `runs`. Stable = relative MAD of the frame totals (median absolute
//! deviation from the median over the median) at most `stable_mad`:
//! a steady page stops at `min_runs` frames, a noisy one runs to the cap.

use std::time::Duration;

#[derive(Clone, Copy)]
pub(super) struct Series {
    max: usize,
    min: usize,
    budget: Duration,
    stable: f64,
}

/// Frame total (`first_frame_total`) of a sample: the value stability is judged by.
pub(super) fn total(sample: &super::report::Sample) -> Option<f64> {
    sample.get("first_frame_total").copied().flatten()
}

/// Relative MAD; `None` for fewer than two values or a zero median.
pub(super) fn relative_mad(values: &[f64]) -> Option<f64> {
    fn median(v: &mut [f64]) -> f64 {
        v.sort_by(f64::total_cmp);
        let n = v.len();
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.
        }
    }
    if values.len() < 2 {
        return None;
    }
    let mut v = values.to_vec();
    let m = median(&mut v);
    if m <= 0. {
        return None;
    }
    let mut dev: Vec<f64> = values.iter().map(|x| (x - m).abs()).collect();
    Some(median(&mut dev) / m)
}

impl Series {
    pub(super) fn new(args: &super::args::Args) -> Self {
        Self::with(args.runs, args.min_runs, args.series_ms, args.stable_mad)
    }

    pub(super) fn with(runs: usize, min_runs: usize, series_ms: u64, stable_mad: f64) -> Self {
        if series_ms == 0 {
            return Self {
                max: runs,
                min: runs,
                budget: Duration::ZERO,
                stable: 0.,
            };
        }
        let min = min_runs.clamp(1, runs.max(1));
        Self {
            max: runs,
            min,
            budget: Duration::from_millis(series_ms),
            stable: stable_mad,
        }
    }

    /// One more frame after `done` frames that took `spent`, with totals `totals`?
    pub(super) fn more(&self, done: usize, spent: Duration, totals: &[f64]) -> bool {
        done < self.max && (done < self.min || (spent < self.budget && !self.settled(totals)))
    }

    fn settled(&self, totals: &[f64]) -> bool {
        self.stable > 0.
            && totals.len() >= self.min.max(2)
            && relative_mad(totals).is_some_and(|r| r <= self.stable)
    }

    /// Shortest allowed series.
    pub(super) fn min_frames(&self) -> usize {
        self.min
    }

    /// A timed (not fixed) series: its length is known only at its end.
    pub(super) fn timed(&self) -> bool {
        !self.budget.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_series_runs_exactly_the_requested_frames() {
        let s = Series::with(6, 3, 0, 0.03);
        assert!(s.more(5, Duration::from_secs(9), &[1., 1., 1., 1., 1.]));
        assert!(!s.more(6, Duration::ZERO, &[]));
        assert!(!s.timed());
    }

    #[test]
    fn timed_series_needs_both_the_minimum_and_the_duration_but_stops_at_the_maximum() {
        let s = Series::with(30, 3, 500, 0.);
        assert!(s.more(2, Duration::from_secs(5), &[]));
        assert!(s.more(10, Duration::from_millis(499), &[1.; 10]));
        assert!(!s.more(10, Duration::from_millis(500), &[]));
        assert!(!s.more(30, Duration::ZERO, &[]));
        assert!(Series::with(2, 3, 500, 0.).more(1, Duration::from_secs(1), &[]));
        assert!(!Series::with(2, 3, 500, 0.).more(2, Duration::ZERO, &[]));
    }

    #[test]
    fn adaptive_series_stops_once_the_frames_are_stable() {
        let s = Series::with(30, 3, 500, 0.03);
        let early = Duration::from_millis(10);
        assert!(s.more(2, early, &[10., 10.]), "the minimum comes first");
        assert!(
            !s.more(3, early, &[10., 10.1, 9.9]),
            "stable at the minimum"
        );
        assert!(s.more(3, early, &[10., 14., 8.]), "noisy: more frames");
        assert!(
            !s.more(4, early, &[10., 10.1, 30., 10.2]),
            "one outlier does not keep it going"
        );
        assert!(
            !s.more(5, Duration::from_millis(500), &[10., 14., 8., 13., 7.]),
            "the cap still ends it"
        );
        assert!(s.more(3, early, &[]), "no totals: by time as before");
        assert_eq!(relative_mad(&[10., 11., 9.]), Some(0.1));
        assert_eq!(relative_mad(&[5.]), None);
    }
}
