//! Снимки счётчиков завершённого кадра и их дельты.

use gpui::frame_perf;
use kamin_html::host::capture::CaptureSession;
use std::{sync::atomic::Ordering::Relaxed, time::Duration};

pub(super) struct Counters {
    pub(super) draw: u32,
    pub(super) present: u32,
    pub(super) layout: u32,
    pub(super) frames: u32,
    pub(super) readback: Duration,
}

impl Counters {
    pub(super) fn read(capture: &CaptureSession) -> Self {
        Self {
            draw: frame_perf::DRAW_US.load(Relaxed),
            present: frame_perf::PRESENT_US.load(Relaxed),
            layout: frame_perf::TAFFY_US.load(Relaxed),
            frames: frame_perf::FRAMES.load(Relaxed),
            readback: capture.readback_time(),
        }
    }
    pub(super) fn delta(current: u32, previous: u32) -> Duration {
        Duration::from_micros(u64::from(current.wrapping_sub(previous)))
    }
}
