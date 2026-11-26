//! Real presentation and idle boundaries, without pixel readback or inferred frame times.

use super::counters::Counters;
use gpui::profiler::journal::{
    ForegroundJournalCollector, ForegroundJournalEntry as Entry, IntervalBoundary as Boundary,
};
use gpui::{App, Subscription, Window};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Step {
    mark: Option<Instant>,
    total: Duration,
    longest: Duration,
    calls: usize,
}

pub struct Timing {
    pub viewport: (f32, f32),
    pub started: Instant,
    counters: Counters,
    journal: ForegroundJournalCollector,
    pub io_ms: f64,
    pub document_ms: f64,
    pub parse_ms: f64,
    pub style_ms: f64,
    first: Option<Duration>,
    idle: Option<Duration>,
    last_present: Option<Duration>,
    lost: u64,
    presented: usize,
    longest_draw: Duration,
    longest_present: Duration,
    step: Rc<RefCell<Step>>,
    _before: Subscription,
    after: Option<Subscription>,
}

impl Timing {
    pub fn new(viewport: (f32, f32), window: &Window, cx: &App) -> Self {
        let step = Rc::new(RefCell::new(Step::default()));
        let before = step.clone();
        // GPUI SubscriberSet iterates monotonically increasing subscription ids.
        // Registered before DocumentView::open; the matching hook is registered after it.
        let subscription = window.on_update_rendering(move |_, _| {
            before.borrow_mut().mark = Some(Instant::now());
        });
        Self {
            viewport,
            started: Instant::now(),
            counters: Counters::read(),
            journal: cx.foreground_journal().collector(),
            io_ms: 0.,
            document_ms: 0.,
            parse_ms: 0.,
            style_ms: 0.,
            first: None,
            idle: None,
            last_present: None,
            lost: 0,
            presented: 0,
            longest_draw: Duration::ZERO,
            longest_present: Duration::ZERO,
            step,
            _before: subscription,
            after: None,
        }
    }

    pub fn built(&mut self, window: &Window) {
        let step = self.step.clone();
        self.after = Some(window.on_update_rendering(move |_, _| {
            let mut step = step.borrow_mut();
            if let Some(mark) = step.mark.take() {
                let elapsed = mark.elapsed();
                step.total += elapsed;
                step.longest = step.longest.max(elapsed);
                step.calls += 1;
            }
        }));
    }

    pub fn poll(&mut self) -> bool {
        let drained = self.journal.collect_unseen();
        self.lost += drained.lost;
        for entry in drained.entries {
            match entry {
                Entry::Boundary(Boundary::Presented(frame)) => {
                    // A previous page's queued presentation may be published late.
                    if frame.frame.draw_start.elapsed() > self.started.elapsed() {
                        continue;
                    }
                    self.presented += 1;
                    self.longest_draw = self.longest_draw.max(frame.frame.draw_duration());
                    self.longest_present = self
                        .longest_present
                        .max(frame.presentation.present_duration());
                    // Both clocks are monotonic elapsed clocks; no extra scheduler dependency.
                    let at = self
                        .started
                        .elapsed()
                        .saturating_sub(frame.presentation.present_end.elapsed());
                    self.first.get_or_insert(at);
                    self.last_present = Some(at);
                    self.idle = None;
                }
                Entry::Boundary(Boundary::Idle { ended_at }) if self.first.is_some() => {
                    let at = self.started.elapsed().saturating_sub(ended_at.elapsed());
                    if self.last_present.is_some_and(|last| at >= last) {
                        self.idle.get_or_insert(at);
                    }
                }
                _ => {}
            }
        }
        self.idle.is_some()
            && self.last_present.is_some_and(|last| {
                self.started.elapsed().saturating_sub(last) >= Duration::from_millis(250)
            })
    }

    pub fn report(
        &self,
        path: &std::path::Path,
        error: Option<&str>,
        retained: Option<(u64, u64)>,
        inside_draw: Option<u64>,
        settled: bool,
    ) -> serde_json::Value {
        let step = self.step.borrow();
        let other = self.document_ms - self.parse_ms - self.style_ms;
        serde_json::json!({"page": path, "viewport": self.viewport,
            "first_present_ms": self.first.map(ms), "idle_ms": self.idle.map(ms),
            "settled": settled, "journal_lost": self.lost, "error": error,
            "presented_frames": self.presented,
            "longest_draw_without_document_step_ms": ms(self.longest_draw),
            "longest_raster_submit_ms": ms(self.longest_present),
            "io_ms": self.io_ms, "document_ms": self.document_ms,
            "parse_ms": self.parse_ms, "style_ms": self.style_ms,
            "document_other_ms": (other >= 0.).then_some(other),
            "document_phase_accounting_valid": other >= 0.,
            "document_step_ms": ms(step.total), "longest_document_step_ms": ms(step.longest),
            "document_step_calls": step.calls, "retained_builds_replays": retained,
            "builds_inside_draw": inside_draw, "gpui": self.counters.delta()})
    }
}

pub fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.
}
