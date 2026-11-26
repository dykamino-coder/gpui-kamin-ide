//! Existing GPUI presentation observations in the DevTools frame protocol.

use super::{Instant, Value, json, ms};
use gpui::profiler::{PresentTiming, journal::PresentedFrame};

pub(super) fn fields(
    first: bool,
    requested: Instant,
    previous: Option<PresentTiming>,
    frame: PresentedFrame,
) -> Value {
    let present = frame.presentation;
    json!({
        "presented": true, "presentation_boundary": "platform_submit_complete",
        "first_present_ms": first.then(|| ms(present.present_end.duration_since(requested))),
        "present_interval_ms": previous.map(|last| ms(present.present_end.duration_since(last.present_end))),
        "raster_submit_ms": ms(present.present_duration()),
        "dirty_to_present_ms": frame.dirty_to_present_duration().map(ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{WindowId, profiler::FrameTiming};
    use std::time::Duration;

    #[test]
    fn presentation_intervals_use_the_observed_boundary_and_keep_missing_values() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let window_id = WindowId::from(1);
        let frame = PresentedFrame {
            frame: FrameTiming {
                window_id,
                dirty_at: Some(at(1)),
                invalidations: 2,
                draw_start: at(2),
                draw_end: at(4),
            },
            presentation: PresentTiming {
                window_id,
                present_start: at(5),
                present_end: at(6),
                animation_interval: None,
            },
        };
        let first = fields(true, start, None, frame);
        assert_eq!(first["first_present_ms"], 6.0);
        assert!(first["present_interval_ms"].is_null());
        assert_eq!(first["raster_submit_ms"], 1.0);
        assert_eq!(first["dirty_to_present_ms"], 5.0);
        let next = PresentedFrame {
            presentation: PresentTiming {
                present_start: at(21),
                present_end: at(22),
                ..frame.presentation
            },
            ..frame
        };
        let next = fields(false, start, Some(frame.presentation), next);
        assert_eq!(next["present_interval_ms"], 16.0);
        assert!(next["first_present_ms"].is_null());
        assert_eq!(next["presentation_boundary"], "platform_submit_complete");
    }
}
