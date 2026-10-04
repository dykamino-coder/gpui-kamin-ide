//! Tab intervals belong to inline styles, but their positions share a line origin.

use std::{
    hash::{Hash, Hasher},
    ops::Range,
};

#[derive(Clone)]
pub struct TabStops {
    pub default: f32,
    pub spans: Vec<(Range<usize>, f32)>,
}

impl TabStops {
    pub fn uniform(step: f32) -> Self {
        Self {
            default: step,
            spans: Vec::new(),
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
        (position / step).floor() * step + step
    }

    pub fn hash_into(&self, hasher: &mut impl Hasher) {
        self.default.to_bits().hash(hasher);
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
        };
        let b = TabStops {
            default: 16.0,
            spans: vec![(2..4, 80.0)],
        };
        let c = TabStops {
            default: 16.0,
            spans: vec![(3..4, 40.0)],
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
