//! Tab intervals belong to inline styles, but their positions share a line origin.

use std::{
    hash::{Hash, Hasher},
    ops::Range,
};

#[derive(Clone)]
pub struct TabStops {
    pub default: f32,
    pub spans: Vec<(Range<usize>, f32)>,
    /// Наименьшее расстояние до позиции табуляции — `0.5ch` (css-text-3
    /// §4.2 `tab-size`: «If this distance is less than 0.5ch, then the
    /// subsequent tab stop is used instead»). Ноль — без порога.
    pub min_gap: f32,
}

impl TabStops {
    pub fn uniform(step: f32) -> Self {
        Self {
            default: step,
            spans: Vec::new(),
            min_gap: 0.0,
        }
    }

    pub fn next(&self, byte: usize, position: f32) -> f32 {
        let step = self
            .spans
            .iter()
            .find(|(r, _)| r.contains(&byte))
            .map_or(self.default, |(_, step)| *step);
        if step <= 0.0 {
            return position;
        }
        let stop = (position / step).floor() * step + step;
        // Допуск — от дробей `ch`: `7.5ch` до стопа `8ch` ровно на пороге.
        if stop - position < self.min_gap - 1e-3 {
            stop + step
        } else {
            stop
        }
    }

    pub fn hash_into(&self, hasher: &mut impl Hasher) {
        self.default.to_bits().hash(hasher);
        self.min_gap.to_bits().hash(hasher);
        for (range, step) in &self.spans {
            range.hash(hasher);
            step.to_bits().hash(hasher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_tabs_keep_the_block_origin_and_use_their_own_interval() {
        let stops = TabStops {
            default: 16.0,
            spans: vec![(2..4, 40.0), (4..5, 80.0)],
            min_gap: 0.0,
        };
        assert_eq!(stops.next(0, 0.0), 16.0);
        assert_eq!(stops.next(2, 0.0), 40.0);
        assert_eq!(stops.next(3, 40.0), 80.0);
        assert_eq!(stops.next(4, 0.0), 80.0);
        assert_eq!(stops.next(5, 40.0), 48.0);
    }

    #[test]
    fn interval_and_range_changes_invalidate_measurements() {
        fn hash(stops: &TabStops) -> u64 {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            stops.hash_into(&mut h);
            h.finish()
        }
        let a = TabStops {
            default: 16.0,
            spans: vec![(2..4, 40.0)],
            min_gap: 0.0,
        };
        let b = TabStops {
            default: 16.0,
            spans: vec![(2..4, 80.0)],
            min_gap: 0.0,
        };
        let c = TabStops {
            default: 16.0,
            spans: vec![(3..4, 40.0)],
            min_gap: 0.0,
        };
        assert_ne!(hash(&a), hash(&b));
        assert_ne!(hash(&a), hash(&c));
    }

    #[test]
    fn zero_tabs_disappear_and_subpixel_intervals_are_preserved() {
        assert_eq!(TabStops::uniform(0.0).next(0, 37.0), 37.0);
        assert_eq!(TabStops::uniform(0.25).next(0, 0.3), 0.5);
    }
}

#[cfg(test)]
mod threshold_tests {
    use super::*;

    #[test]
    fn a_stop_closer_than_half_ch_is_skipped() {
        let stops = TabStops {
            min_gap: 5.0,
            ..TabStops::uniform(80.0)
        };
        assert_eq!(stops.next(0, 76.0), 160.0);
        assert_eq!(stops.next(0, 73.0), 80.0);
    }
}
