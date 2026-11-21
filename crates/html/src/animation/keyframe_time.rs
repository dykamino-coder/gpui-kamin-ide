//! Local effect origin on the document's shared frame clock.

use super::clock::{AnimationFrame, AnimationTimeSource};
use crate::style::computed::AnimSpec;
use std::time::Duration;

#[derive(Default)]
pub(super) struct LocalTime {
    name: String,
    source: Option<AnimationTimeSource>,
    start: Duration,
    offset: f64,
    last: f64,
}

impl LocalTime {
    pub(super) fn sample(&mut self, spec: &AnimSpec, frame: AnimationFrame) -> f64 {
        if self.source.is_none() || self.name != spec.name {
            self.name.clone_from(&spec.name);
            self.source = Some(frame.source);
            self.start = frame.elapsed;
            self.offset = 0.0;
            self.last = 0.0;
        } else if self.source != Some(frame.source) {
            self.source = Some(frame.source);
            self.start = frame.elapsed;
            self.offset = self.last;
        }
        self.last = if frame.source == AnimationTimeSource::Live {
            self.offset + frame.elapsed.saturating_sub(self.start).as_secs_f64()
        } else {
            frame.elapsed.as_secs_f64()
        };
        self.last
    }
}

pub(super) fn end(spec: &AnimSpec) -> f64 {
    (f64::from(spec.seconds) * spec.iterations + f64::from(spec.delay)).max(0.0)
}

pub(super) fn needs_frame(
    spec: &AnimSpec,
    frame: AnimationFrame,
    elapsed: f64,
    reduced: bool,
) -> bool {
    frame.source == AnimationTimeSource::Live && !reduced && (spec.infinite || elapsed < end(spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> AnimSpec {
        AnimSpec {
            name: "move".into(),
            seconds: 1.0,
            infinite: false,
            alternate: false,
            delay: 0.0,
            paused: false,
            names: Vec::new(),
            fill: 0,
            iterations: 1.0,
            reverse: false,
        }
    }

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

    #[test]
    fn matching_name_keeps_origin_when_duration_changes_and_new_name_restarts() {
        let mut local = LocalTime::default();
        let mut spec = spec();
        assert_eq!(
            local.sample(&spec, frame(10_000, AnimationTimeSource::Live)),
            0.0
        );
        assert_eq!(
            local.sample(&spec, frame(10_500, AnimationTimeSource::Live)),
            0.5
        );
        spec.seconds = 2.0;
        assert_eq!(
            local.sample(&spec, frame(10_500, AnimationTimeSource::Live)),
            0.5
        );
        assert_eq!(spec.progress_at(0.5), Some(0.25));
        spec.name = "other".into();
        assert_eq!(
            local.sample(&spec, frame(10_500, AnimationTimeSource::Live)),
            0.0
        );
    }

    #[test]
    fn snapshot_ignores_mount_origin_and_seek_is_exact() {
        let mut local = LocalTime::default();
        let spec = spec();
        for ms in [750, 250, 250, 300_000] {
            assert_eq!(
                local.sample(&spec, frame(ms, AnimationTimeSource::DocumentSnapshot)),
                ms as f64 / 1000.0
            );
        }
    }

    #[test]
    fn live_rebase_preserves_snapshot_time_beyond_the_live_clock_lifetime() {
        let mut local = LocalTime::default();
        let spec = spec();
        assert_eq!(
            local.sample(&spec, frame(90_000, AnimationTimeSource::DocumentSnapshot)),
            90.0
        );
        assert_eq!(
            local.sample(&spec, frame(100, AnimationTimeSource::Live)),
            90.0
        );
        assert_eq!(
            local.sample(&spec, frame(600, AnimationTimeSource::Live)),
            90.5
        );
    }

    #[test]
    fn only_active_live_effects_request_another_frame() {
        let mut spec = spec();
        let live = frame(0, AnimationTimeSource::Live);
        assert!(needs_frame(&spec, live, 0.5, false));
        assert!(!needs_frame(&spec, live, 1.0, false));
        assert!(!needs_frame(&spec, live, 0.5, true));
        spec.delay = 2.0;
        assert!(needs_frame(&spec, live, 2.5, false));
        spec.infinite = true;
        assert!(needs_frame(&spec, live, 100_000.0, false));
        for source in [
            AnimationTimeSource::DocumentSnapshot,
            AnimationTimeSource::LegacySnapshot,
        ] {
            assert!(!needs_frame(&spec, frame(500, source), 0.5, false));
        }
    }
}
