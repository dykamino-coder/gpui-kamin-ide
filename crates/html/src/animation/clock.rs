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
    /// Matched keyframes or an in-progress transition consume the frame time.
    pub time_dependent: bool,
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
    sampled_effect: bool,
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

    #[cfg(test)]
    pub(crate) fn retained_key(&self) -> (Option<Duration>, u64) {
        let state = self.state.borrow();
        (state.elapsed, state.generation)
    }

    /// Static documents keep their render key when benchmark time advances.
    /// After observing an effect, conservatively retain the dependency for this document.
    pub(crate) fn render_key(&self) -> (Option<Duration>, u64) {
        let state = self.state.borrow();
        if state.sampled_effect {
            (state.elapsed, state.generation)
        } else {
            (None, 0)
        }
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
                time_dependent: state.sampled_effect,
                keyframe_requested: false,
                transition_requested: false,
                hover_target_changed: false,
            });
        }
        state.frame.expect("sample initializes the frame")
    }

    pub(crate) fn keyframe(&self, requested: bool) {
        let mut state = self.state.borrow_mut();
        state.sampled_effect = true;
        if let Some(frame) = &mut state.frame {
            frame.keyframe_samples += 1;
            frame.time_dependent = true;
            frame.keyframe_requested |= requested;
        }
    }

    pub(crate) fn transition(&self, active: bool, requested: bool) {
        let mut state = self.state.borrow_mut();
        state.sampled_effect |= active;
        if let Some(frame) = &mut state.frame {
            frame.time_dependent |= active;
            frame.transition_samples += 1;
            frame.transition_requested |= requested;
        }
    }

    pub(crate) fn hover_changed(&self) {
        let mut state = self.state.borrow_mut();
        state.sampled_effect = true;
        if let Some(frame) = &mut state.frame {
            frame.time_dependent = true;
            frame.hover_target_changed = true;
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn test_frame(ms: u64, source: AnimationTimeSource) -> AnimationFrame {
    AnimationFrame {
        window_id: 1,
        frame_serial: ms,
        source,
        elapsed: Duration::from_millis(ms),
        keyframe_samples: 0,
        transition_samples: 0,
        time_dependent: false,
        keyframe_requested: false,
        transition_requested: false,
        hover_target_changed: false,
    }
}
