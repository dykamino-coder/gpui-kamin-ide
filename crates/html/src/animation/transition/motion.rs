//! Analytic progress of the existing linear hover transition.

use crate::animation::clock::{AnimationFrame, AnimationTimeSource};
use std::time::Duration;

#[derive(Default)]
pub(super) struct Motion {
    pub(super) hovered: bool,
    pub(super) progress: f32,
    start: Duration,
    start_progress: f32,
    source: Option<AnimationTimeSource>,
}

impl Motion {
    pub(super) fn sample(&mut self, frame: AnimationFrame, seconds: f32, reduced: bool) -> bool {
        if self.source != Some(frame.source) {
            self.start = frame.elapsed;
            self.start_progress = self.progress;
            self.source = Some(frame.source);
        }
        let target = if self.hovered { 1.0 } else { 0.0 };
        let distance = target - self.start_progress;
        let step =
            frame.elapsed.saturating_sub(self.start).as_secs_f64() / f64::from(seconds.max(0.01));
        self.progress = if reduced {
            self.start = frame.elapsed;
            self.start_progress = target;
            target
        } else {
            self.start_progress + distance.signum() * (step.min(f64::from(distance.abs())) as f32)
        };
        self.progress != target
    }

    pub(super) fn target(&mut self, hovered: bool, frame: AnimationFrame) -> bool {
        if self.hovered == hovered {
            return false;
        }
        self.hovered = hovered;
        self.start = frame.elapsed;
        self.start_progress = self.progress;
        self.source = Some(frame.source);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ms: u64, source: AnimationTimeSource) -> AnimationFrame {
        AnimationFrame {
            window_id: 1,
            frame_serial: ms,
            source,
            elapsed: Duration::from_millis(ms),
            keyframe_samples: 0,
            transition_samples: 0,
            keyframe_requested: false,
            transition_requested: false,
            hover_target_changed: false,
        }
    }

    fn at(ms: u64) -> AnimationFrame {
        frame(ms, AnimationTimeSource::DocumentSnapshot)
    }

    #[test]
    fn reversal_uses_the_remaining_distance_and_same_frame_is_stationary() {
        let mut state = Motion::default();
        state.sample(at(100), 1.0, false);
        assert!(state.target(true, at(100)));
        assert!(state.sample(at(500), 1.0, false));
        assert_eq!(state.progress, 0.4);
        state.sample(at(500), 1.0, false);
        assert_eq!(state.progress, 0.4);
        assert!(state.target(false, at(500)));
        state.sample(at(700), 1.0, false);
        assert!((state.progress - 0.2).abs() < 0.00001);
        assert!(!state.sample(at(900), 1.0, false));
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn seeks_are_analytic_and_long_pauses_complete_without_clamping_dt() {
        let mut state = Motion::default();
        state.sample(at(1000), 1.0, false);
        state.target(true, at(1000));
        state.sample(at(1800), 1.0, false);
        assert_eq!(state.progress, 0.8);
        state.sample(at(1200), 1.0, false);
        assert_eq!(state.progress, 0.2);
        state.sample(at(500), 1.0, false);
        assert_eq!(state.progress, 0.0);
        assert!(!state.sample(at(120_000), 1.0, false));
        assert_eq!(state.progress, 1.0);
    }

    #[test]
    fn changing_clock_source_rebases_without_jumping_progress() {
        let mut state = Motion::default();
        state.sample(at(0), 1.0, false);
        state.target(true, at(0));
        state.sample(at(400), 1.0, false);
        state.sample(frame(90_000, AnimationTimeSource::Live), 1.0, false);
        assert_eq!(state.progress, 0.4);
        state.sample(frame(90_300, AnimationTimeSource::Live), 1.0, false);
        assert!((state.progress - 0.7).abs() < 0.00001);
    }

    #[test]
    fn reduced_motion_snaps_to_target_without_requesting_more_progress() {
        let mut state = Motion::default();
        state.sample(at(0), 1.0, false);
        state.target(true, at(0));
        assert!(!state.sample(at(1), 1.0, true));
        assert_eq!(state.progress, 1.0);
        assert!(!state.sample(at(2), 1.0, false));
        assert_eq!(state.progress, 1.0);
    }
}
