//! One animation timestamp per document/window frame, plus bounded diagnostics.

use gpui::{App, Window};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

/// Source of the timestamp shared by CSS effects in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationTimeSource {
    Live,
    DocumentSnapshot,
    LegacySnapshot,
}

impl AnimationTimeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::DocumentSnapshot => "document_snapshot",
            Self::LegacySnapshot => "legacy_snapshot",
        }
    }
}

/// Animation work observed in the current frame. Reading this never schedules work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationFrame {
    pub window_id: u64,
    pub frame_serial: u64,
    pub source: AnimationTimeSource,
    pub elapsed: Duration,
    pub keyframe_samples: u64,
    pub transition_samples: u64,
    pub keyframe_requested: bool,
    pub transition_requested: bool,
    pub hover_target_changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    window: u64,
    serial: u64,
    input: Option<(AnimationTimeSource, Duration)>,
}

#[derive(Default)]
struct State {
    elapsed: Option<Duration>,
    generation: u64,
    key: Option<Key>,
    frame: Option<AnimationFrame>,
}

pub(crate) struct Clock {
    origin: Instant,
    state: RefCell<State>,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            state: RefCell::default(),
        }
    }
}

impl Clock {
    pub(crate) fn set_elapsed(&self, elapsed: Option<Duration>) {
        let mut state = self.state.borrow_mut();
        if state.elapsed != elapsed {
            state.elapsed = elapsed;
            state.generation = state.generation.wrapping_add(1);
            state.key = None;
            state.frame = None;
        }
    }

    pub(crate) fn retained_key(&self) -> (Option<Duration>, u64) {
        let state = self.state.borrow();
        (state.elapsed, state.generation)
    }

    pub(crate) fn frame(&self) -> Option<AnimationFrame> {
        self.state.borrow().frame
    }

    pub(crate) fn sample(&self, window: &Window, cx: &App) -> AnimationFrame {
        self.sample_with(
            window.window_handle().window_id().as_u64(),
            window.frame_serial(),
            cx.try_global::<gpui::AnimationElapsedTime>().map(|t| t.0),
            || self.origin.elapsed(),
        )
    }

    fn sample_with(
        &self,
        window: u64,
        serial: u64,
        legacy: Option<Duration>,
        live: impl FnOnce() -> Duration,
    ) -> AnimationFrame {
        let mut state = self.state.borrow_mut();
        let input = state
            .elapsed
            .map(|t| (AnimationTimeSource::DocumentSnapshot, t))
            .or_else(|| legacy.map(|t| (AnimationTimeSource::LegacySnapshot, t)));
        let key = Key {
            window,
            serial,
            input,
        };
        if state.key != Some(key) {
            let (source, elapsed) = input.unwrap_or_else(|| (AnimationTimeSource::Live, live()));
            state.key = Some(key);
            state.frame = Some(AnimationFrame {
                window_id: window,
                frame_serial: serial,
                source,
                elapsed,
                keyframe_samples: 0,
                transition_samples: 0,
                keyframe_requested: false,
                transition_requested: false,
                hover_target_changed: false,
            });
        }
        state.frame.expect("sample initializes the frame")
    }

    pub(crate) fn keyframe(&self, requested: bool) {
        if let Some(frame) = &mut self.state.borrow_mut().frame {
            frame.keyframe_samples += 1;
            frame.keyframe_requested |= requested;
        }
    }

    pub(crate) fn transition(&self, requested: bool) {
        if let Some(frame) = &mut self.state.borrow_mut().frame {
            frame.transition_samples += 1;
            frame.transition_requested |= requested;
        }
    }

    pub(crate) fn hover_changed(&self) {
        if let Some(frame) = &mut self.state.borrow_mut().frame {
            frame.hover_target_changed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn repeated_layout_reads_one_timestamp_and_keeps_diagnostics() {
        let clock = Clock::default();
        assert_eq!(clock.sample_with(1, 5, None, || ms(10)).elapsed, ms(10));
        clock.keyframe(true);
        let frame = clock.sample_with(1, 5, None, || panic!("second clock read"));
        assert_eq!(frame.elapsed, ms(10));
        assert_eq!(frame.keyframe_samples, 1);
        assert!(frame.keyframe_requested);
        assert_eq!(clock.frame(), Some(frame));
        let next = clock.sample_with(1, 6, None, || ms(30));
        assert_eq!(next.elapsed, ms(30));
        assert_eq!(next.keyframe_samples, 0);
        assert!(!next.keyframe_requested);
    }

    #[test]
    fn window_identity_and_documents_do_not_share_cached_samples() {
        let clock = Clock::default();
        assert_eq!(clock.sample_with(1, 1, None, || ms(10)).elapsed, ms(10));
        assert_eq!(clock.sample_with(2, 1, None, || ms(15)).elapsed, ms(15));
        let other = Clock::default();
        other.set_elapsed(Some(ms(99)));
        assert_eq!(other.sample_with(1, 1, None, || panic!()).elapsed, ms(99));
        assert_eq!(clock.frame().unwrap().elapsed, ms(15));
    }

    #[test]
    fn snapshot_precedence_seek_and_retained_invalidation_are_explicit() {
        let clock = Clock::default();
        let before = clock.retained_key();
        clock.set_elapsed(Some(ms(900)));
        assert_ne!(before, clock.retained_key());
        let same = clock.retained_key();
        clock.set_elapsed(Some(ms(900)));
        assert_eq!(same, clock.retained_key());
        let frame = clock.sample_with(1, 1, Some(ms(200)), || panic!());
        assert_eq!(frame.source, AnimationTimeSource::DocumentSnapshot);
        assert_eq!(frame.elapsed, ms(900));
        clock.set_elapsed(Some(ms(10)));
        assert_eq!(clock.sample_with(1, 1, None, || panic!()).elapsed, ms(10));
        clock.set_elapsed(None);
        let legacy = clock.sample_with(1, 1, Some(ms(200)), || panic!());
        assert_eq!(legacy.source, AnimationTimeSource::LegacySnapshot);
        assert_eq!(legacy.elapsed, ms(200));
        assert_eq!(
            clock.sample_with(1, 1, None, || ms(300)).source,
            AnimationTimeSource::Live
        );
    }

    #[test]
    fn fixed_time_is_held_across_frames_and_reading_does_not_request_frames() {
        let clock = Clock::default();
        clock.set_elapsed(Some(Duration::ZERO));
        for serial in 0..3 {
            let frame = clock.sample_with(1, serial, None, || panic!());
            assert_eq!(frame.elapsed, Duration::ZERO);
            assert!(!frame.keyframe_requested && !frame.transition_requested);
            clock.transition(false);
            clock.hover_changed();
            assert!(clock.frame().unwrap().hover_target_changed);
        }
    }
}
