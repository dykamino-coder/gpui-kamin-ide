//! Presented frames of this window, read from the engine's foreground journal.
//! Draining the journal never requests a frame.

use gpui::{
    App, Window, WindowId,
    profiler::{
        Instant, TraceGuard,
        journal::{ForegroundJournalCollector, ForegroundJournalEntry as Entry, IntervalBoundary},
    },
};
use std::time::Duration;

pub struct Frames {
    collector: ForegroundJournalCollector,
    window: WindowId,
    last_present: Option<Instant>,
    _trace: TraceGuard,
}

#[derive(Default)]
pub struct Sample {
    pub presented: u32,
    pub skipped: u32,
    pub lost: u64,
    pub draw: Span,
    pub present: Span,
    pub interval: Span,
    pub dirty_to_present: Span,
}

#[derive(Default, Clone, Copy)]
pub struct Span {
    total: Duration,
    max: Duration,
    count: u32,
}

impl Span {
    fn add(&mut self, duration: Duration) {
        self.total += duration;
        self.max = self.max.max(duration);
        self.count += 1;
    }

    /// Mean and maximum in milliseconds, or dashes when nothing was observed.
    pub fn text(&self) -> String {
        if self.count == 0 {
            return "- / -".into();
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        format!(
            "{:.1} / {:.1}",
            ms(self.total) / f64::from(self.count),
            ms(self.max)
        )
    }
}

impl Frames {
    pub fn new(window: &Window, cx: &App) -> Self {
        Self {
            _trace: gpui::profiler::trace_scope(),
            collector: cx.foreground_journal().collector(),
            window: window.window_handle().window_id(),
            last_present: None,
        }
    }

    pub fn drain(&mut self) -> Sample {
        let batch = self.collector.collect_unseen();
        let mut sample = Sample {
            lost: batch.lost,
            ..Default::default()
        };
        for entry in batch.entries {
            match entry {
                Entry::Boundary(IntervalBoundary::Presented(frame))
                    if frame.frame.window_id == self.window =>
                {
                    sample.presented += 1;
                    let draw = frame.frame;
                    sample
                        .draw
                        .add(draw.draw_end.duration_since(draw.draw_start));
                    let present = frame.presentation;
                    sample
                        .present
                        .add(present.present_end.duration_since(present.present_start));
                    if let Some(total) = frame.dirty_to_present_duration() {
                        sample.dirty_to_present.add(total);
                    }
                    if let Some(last) = self.last_present {
                        sample
                            .interval
                            .add(present.present_end.duration_since(last));
                    }
                    self.last_present = Some(present.present_end);
                }
                Entry::Boundary(IntervalBoundary::FrameSkipped(_)) => sample.skipped += 1,
                // A pause between bursts is not a frame interval.
                Entry::Boundary(IntervalBoundary::Idle { .. }) => self.last_present = None,
                _ => {}
            }
        }
        sample
    }
}
