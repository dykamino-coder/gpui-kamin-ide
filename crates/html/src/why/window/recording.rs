//! Mutable state of one explicitly enabled shared recorder.

use super::idle::{IdleCpuWindow, Observation};
use super::{Instant, Report, Step, Value, activity::Activity, json, ms, presentation};
use crate::Document;
use gpui::profiler::journal::{
    ForegroundJournalCollector, ForegroundJournalEntry as Entry, IntervalBoundary,
};
use gpui::{App, Window};
use std::{collections::VecDeque, path::Path, string::String};

/// Opt-in recorder shared by embedded hosts. Dropping it releases its why
/// subscription; hosts without it perform no journal sampling or heap walk.
pub(super) struct Recording {
    report: Report,
    collector: ForegroundJournalCollector,
    window: gpui::WindowId,
    pending: VecDeque<(Instant, Instant, usize)>,
    drawn: Vec<(gpui::profiler::FrameTiming, usize)>,
    previous: Option<gpui::profiler::PresentTiming>,
    first: bool,
    build_pending: bool,
    requested: Instant,
    fields: Value,
    errors: Vec<String>,
    lost: u64,
    request: u64,
    idle: Option<Observation>,
    activity: Activity,
    _trace: gpui::profiler::TraceGuard,
}

impl Recording {
    pub fn new(window: &Window, cx: &App) -> Self {
        let trace = gpui::profiler::trace_scope();
        Self {
            report: Report::new(),
            collector: cx.foreground_journal().collector(),
            window: window.window_handle().window_id(),
            pending: VecDeque::new(),
            drawn: Vec::new(),
            previous: None,
            first: true,
            build_pending: true,
            requested: Instant::now(),
            fields: json!({"mode": "viewer", "step": "open"}),
            errors: Vec::new(),
            lost: 0,
            request: 0,
            idle: None,
            activity: Activity::default(),
            _trace: trace,
        }
    }

    /// Start one open request. Scenario labels are annotations, not meters.
    pub fn open_document(&mut self, path: &Path) {
        self.poll();
        self.request += 1;
        self.requested = Instant::now();
        self.first = true;
        self.build_pending = true;
        self.previous = None;
        self.fields =
            json!({"mode": "viewer", "step": "open", "page": path, "request_id": self.request});
    }

    pub fn step(&mut self, name: &str, index: usize) {
        self.request += 1;
        self.fields["request_id"] = json!(self.request);
        self.fields["step"] = json!(name);
        self.fields["step_index"] = json!(index);
        self.fields.as_object_mut().unwrap().remove("wheel_changed");
        self.fields.as_object_mut().unwrap().remove("phases_ms");
    }

    /// Labels supplied by a scenario are recorded beside the engine facts.
    pub fn metadata(&mut self, fields: Value) {
        for (key, value) in fields.as_object().into_iter().flatten() {
            self.fields[key] = value.clone();
        }
    }

    /// Observe host loading through this journal, with no separate host timer.
    pub fn phase<T>(&mut self, name: &str, action: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = action();
        self.fields["phases_ms"][name] = json!(ms(start.elapsed()));
        result
    }

    pub fn error(&mut self, message: String) {
        self.errors.push(message);
    }

    pub(crate) fn begin(&mut self, doc: &Document) -> Step {
        self.poll();
        Step::new(self.report.watch(doc), self.fields.clone())
    }

    pub fn sample_memory(&mut self, reason: &str, doc: &Document, window: &Window, cx: &App) {
        self.report.sample_memory(reason, doc, window, cx, None);
    }

    pub(crate) fn drawn(&mut self, step: Step, doc: &Document) {
        let (watch, mut fields, start, end) = step.finish();
        if self.build_pending {
            let build = doc.build_timings();
            fields["phases_ms"]["parse"] = json!(ms(build.parse));
            fields["phases_ms"]["style"] = json!(ms(build.style));
            self.build_pending = false;
        }
        match self.report.record(watch, doc, false, fields) {
            Ok(index) => self.pending.push_back((start, end, index)),
            Err(error) => self.error(error.into()),
        }
    }

    /// Drain observed draws/presentations. Calling this never requests a frame.
    pub fn poll(&mut self) -> usize {
        let batch = self.collector.collect_unseen();
        self.lost += batch.lost;
        self.activity.lost(batch.lost);
        if let Some(idle) = &mut self.idle {
            idle.lost(batch.lost);
        }
        for event in batch.entries {
            self.observe(event);
        }
        self.presented()
    }

    fn observe(&mut self, event: Entry) {
        self.activity.observe(event, self.window);
        if let Some(idle) = &mut self.idle {
            idle.observe(event, self.window);
        }
        match event {
            Entry::Event(gpui::profiler::journal::ForegroundEvent::Draw(frame))
                if frame.window_id == self.window =>
            {
                if let Some(at) = self.pending.iter().position(|(start, end, _)| {
                    frame.draw_start >= *start && frame.draw_end <= *end
                }) {
                    let (_, _, index) = self.pending.remove(at).expect("matched draw");
                    self.report.annotate(
                        index,
                        json!({"phases_ms": {
                            "draw_without_document_step": ms(frame.draw_duration()),
                        }}),
                    );
                    self.drawn.push((frame, index));
                }
            }
            Entry::Boundary(IntervalBoundary::Presented(frame))
                if frame.frame.window_id == self.window =>
            {
                if let Some(at) = self
                    .drawn
                    .iter()
                    .position(|(draw, _)| draw.draw_start == frame.frame.draw_start)
                {
                    let (_, index) = self.drawn.remove(at);
                    let present = frame.presentation;
                    self.report.annotate(
                        index,
                        presentation::fields(self.first, self.requested, self.previous, frame),
                    );
                    self.previous = Some(present);
                    self.first = false;
                }
            }
            _ => {}
        }
    }

    pub fn step_presented(&mut self) -> bool {
        self.poll();
        self.report
            .frames()
            .iter()
            .any(|frame| requested_presentation(frame, self.request))
    }

    pub fn begin_idle(&mut self) {
        self.poll();
        self.idle = Some(Observation::new());
    }

    pub fn finish_idle(&mut self, cpu: IdleCpuWindow) -> Result<(), &'static str> {
        self.poll();
        let sample = self
            .idle
            .take()
            .ok_or("idle observation was not started")?
            .finish(cpu)?;
        self.report.idle_sample(sample);
        Ok(())
    }

    pub fn presented(&self) -> usize {
        self.report
            .frames()
            .iter()
            .filter(|f| f["presented"] == true)
            .count()
    }

    pub fn write(&mut self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.poll();
        self.report
            .status(json!({"foreground_entries_lost": self.lost,
            "draws_awaiting_observation": self.pending.len(),
            "draws_without_presentation": self.drawn.len(), "errors": self.errors,
            "platform_activity": self.activity.to_json()}));
        self.report.write(dir)
    }

    pub fn to_json(&mut self) -> Value {
        self.poll();
        self.report.to_json()
    }
}

fn requested_presentation(frame: &Value, request: u64) -> bool {
    frame["presented"] == true && frame["request_id"].as_u64() == Some(request)
}

#[cfg(test)]
mod tests;
