//! Phase boundaries of the document's existing rendering step.

use super::{Instant, Value, Watch, json, ms};
use crate::UpdatePhase;

pub(crate) struct Step {
    watch: Watch,
    start: Instant,
    mark: Instant,
    phases: serde_json::Map<String, Value>,
    fields: Value,
    calls: u8,
}

impl Step {
    pub(super) fn new(watch: Watch, fields: Value) -> Self {
        let start = Instant::now();
        Self {
            watch,
            start,
            mark: start,
            phases: fields["phases_ms"].as_object().cloned().unwrap_or_default(),
            fields,
            calls: 0,
        }
    }

    pub(crate) fn phase(&mut self, phase: UpdatePhase) {
        let name = match phase {
            UpdatePhase::Built => "build_elements",
            UpdatePhase::Requested => "request_layout",
            UpdatePhase::LaidOut => "compute_layout",
        };
        self.phases
            .insert(name.into(), json!(ms(self.mark.elapsed())));
        self.mark = Instant::now();
    }

    pub(crate) fn prepared(&mut self) {
        self.calls = 1;
        self.phases
            .insert("document_step".into(), json!(ms(self.start.elapsed())));
    }

    pub(super) fn finish(mut self) -> (Watch, Value, Instant, Instant) {
        self.fields["phases_ms"] = json!(self.phases);
        self.fields["document_step_calls"] = json!(self.calls);
        self.fields["presented"] = json!(false);
        (self.watch, self.fields, self.start, Instant::now())
    }
}
