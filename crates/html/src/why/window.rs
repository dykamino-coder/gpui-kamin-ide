//! One shared schema-1 recorder; hosts clone the handle, not its mutable state.

mod activity;
mod idle;
mod presentation;
mod recording;
mod step;
use super::{Report, Watch};
use crate::Document;
use gpui::{App, Window, profiler::Instant};
pub use idle::IdleCpuWindow;
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, rc::Rc};
pub(crate) use step::Step;

/// Explicitly enable observations for one host window. A missing handle costs
/// no clock reads, checkpoints or owner walks in DocumentView.
#[derive(Clone)]
pub struct WindowJournal {
    recording: Rc<RefCell<recording::Recording>>,
}

impl WindowJournal {
    pub fn new(window: &Window, cx: &App) -> Self {
        Self {
            recording: Rc::new(RefCell::new(recording::Recording::new(window, cx))),
        }
    }

    pub fn open_document(&self, path: &Path) {
        self.recording.borrow_mut().open_document(path);
    }

    pub fn step(&self, name: &str, index: usize) {
        self.recording.borrow_mut().step(name, index);
    }

    pub fn metadata(&self, fields: Value) {
        self.recording.borrow_mut().metadata(fields);
    }

    pub fn phase<T>(&self, name: &str, action: impl FnOnce() -> T) -> T {
        self.recording.borrow_mut().phase(name, action)
    }

    pub fn error(&self, message: String) {
        self.recording.borrow_mut().error(message);
    }

    pub(crate) fn begin(&self, doc: &Document) -> Step {
        self.recording.borrow_mut().begin(doc)
    }

    pub(crate) fn drawn(&self, step: Step, doc: &Document) {
        self.recording.borrow_mut().drawn(step, doc);
    }

    /// Take owners on open, an explicit DevTools request or series completion.
    /// This can walk the document and must be called outside frame recording.
    pub fn sample_memory(&self, reason: &str, doc: &Document, window: &Window, cx: &App) {
        self.recording
            .borrow_mut()
            .sample_memory(reason, doc, window, cx);
    }

    /// Drain observed entries without requesting a frame.
    pub fn poll(&self) -> usize {
        self.recording.borrow_mut().poll()
    }

    /// Start a quiet observation without scheduling any polling or frame.
    pub fn begin_idle(&self) {
        self.recording.borrow_mut().begin_idle();
    }

    /// End after an external process observer has read both CPU checkpoints.
    pub fn finish_idle(&self, cpu: IdleCpuWindow) -> Result<(), &'static str> {
        self.recording.borrow_mut().finish_idle(cpu)
    }

    /// Whether the current request has reached platform submit completion.
    pub fn step_presented(&self) -> bool {
        self.recording.borrow_mut().step_presented()
    }

    pub fn write(&self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.recording.borrow_mut().write(dir)
    }

    pub fn to_json(&self) -> Value {
        self.recording.borrow_mut().to_json()
    }
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.
}
