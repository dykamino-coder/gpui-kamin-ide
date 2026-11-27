//! Counts of the existing foreground journal, scoped to one host window.

use gpui::profiler::journal::{
    ForegroundEvent, ForegroundJournalEntry as Entry, FrameSkipReason, IntervalBoundary,
};
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct Activity {
    callbacks: u64,
    draws: u64,
    presentations: u64,
    skips: [u64; 3],
    reentrant: u64,
    lost: u64,
}

impl Activity {
    pub fn lost(&mut self, count: u64) {
        self.lost += count;
    }

    pub fn observe(&mut self, entry: Entry, window: gpui::WindowId) {
        match entry {
            Entry::PlatformSignal(signal) if signal.window_id == window => self.callbacks += 1,
            Entry::Event(ForegroundEvent::Draw(draw)) if draw.window_id == window => {
                self.draws += 1
            }
            Entry::Event(ForegroundEvent::Present(present)) if present.window_id == window => {
                self.presentations += 1
            }
            Entry::Boundary(IntervalBoundary::FrameSkipped(skip)) if skip.window_id == window => {
                let reason = match skip.reason {
                    FrameSkipReason::NoRenderNeeded => 0,
                    FrameSkipReason::InactiveFrameRateLimit => 1,
                    FrameSkipReason::ThermalFrameRateLimit => 2,
                };
                self.skips[reason] += 1;
            }
            Entry::ReentrantFrameSkipped { window_id, .. } if window_id == window => {
                self.reentrant += 1
            }
            _ => {}
        }
    }

    pub fn quiet(&self) -> bool {
        self.callbacks == 0
            && self.draws == 0
            && self.presentations == 0
            && self.skips == [0; 3]
            && self.reentrant == 0
            && self.lost == 0
    }

    pub fn to_json(&self) -> Value {
        json!({
            "platform_callbacks": self.callbacks, "draws": self.draws,
            "presentations": self.presentations, "frame_skips": self.skips.iter().sum::<u64>(),
            "frame_skip_reasons": {"no_render_needed": self.skips[0],
                "inactive_frame_rate_limit": self.skips[1], "thermal_frame_rate_limit": self.skips[2]},
            "reentrant_callbacks": self.reentrant, "foreground_entries_lost": self.lost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        WindowId,
        profiler::{
            FrameTiming, Instant, PresentTiming,
            journal::{FrameSkipped, PlatformSignal, PresentedFrame},
        },
    };

    #[test]
    fn activity_keeps_platform_events_and_reasons_without_counting_other_windows() {
        let mut activity = Activity::default();
        let window = WindowId::from(1);
        let other = WindowId::from(2);
        let now = Instant::now();
        let frame = FrameTiming {
            window_id: window,
            dirty_at: None,
            invalidations: 0,
            draw_start: now,
            draw_end: now,
        };
        let entries = [
            Entry::PlatformSignal(PlatformSignal {
                window_id: window,
                signal_at: None,
                handled_at: now,
                source: gpui::FrameRequestSource::NativeCallback,
            }),
            Entry::Event(ForegroundEvent::Draw(frame)),
            Entry::Event(ForegroundEvent::Present(PresentTiming {
                window_id: window,
                present_start: now,
                present_end: now,
                animation_interval: None,
            })),
            Entry::Boundary(IntervalBoundary::Presented(PresentedFrame {
                frame,
                presentation: PresentTiming {
                    window_id: window,
                    present_start: now,
                    present_end: now,
                    animation_interval: None,
                },
            })),
            Entry::Boundary(IntervalBoundary::FrameSkipped(FrameSkipped {
                window_id: window,
                at: now,
                reason: FrameSkipReason::NoRenderNeeded,
            })),
            Entry::Boundary(IntervalBoundary::FrameSkipped(FrameSkipped {
                window_id: window,
                at: now,
                reason: FrameSkipReason::InactiveFrameRateLimit,
            })),
            Entry::Boundary(IntervalBoundary::FrameSkipped(FrameSkipped {
                window_id: window,
                at: now,
                reason: FrameSkipReason::ThermalFrameRateLimit,
            })),
            Entry::ReentrantFrameSkipped {
                window_id: window,
                at: now,
            },
        ];
        for entry in entries {
            activity.observe(entry, other);
        }
        assert!(activity.quiet());
        for entry in entries {
            activity.observe(entry, window);
        }
        activity.lost(2);
        let result = activity.to_json();
        assert_eq!(result["platform_callbacks"], 1);
        assert_eq!(result["draws"], 1);
        assert_eq!(result["presentations"], 1);
        assert_eq!(result["frame_skips"], 3);
        assert_eq!(result["frame_skip_reasons"]["no_render_needed"], 1);
        assert_eq!(result["frame_skip_reasons"]["inactive_frame_rate_limit"], 1);
        assert_eq!(result["frame_skip_reasons"]["thermal_frame_rate_limit"], 1);
        assert_eq!(result["reentrant_callbacks"], 1);
        assert_eq!(result["foreground_entries_lost"], 2);
        assert!(!activity.quiet());
    }
}
