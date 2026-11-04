//! История фаз и диагностики: между кадрами владеет только дисковым журналом.

use super::{accounting::Measured, journal::Journal, report};
use serde_json::{Value, json};
use std::io;

pub(super) struct History {
    records: Journal,
    metadata: Value,
}

impl History {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self {
            records: Journal::new()?,
            metadata: json!({}),
        })
    }

    pub(super) fn push(
        &mut self,
        measured: Measured,
        work: Value,
        resources: Value,
    ) -> io::Result<()> {
        let mut record = json!({"phases": measured.phases, "diagnostics": measured.diagnostics});
        record["work"] = work;
        record["resources"] = resources;
        self.records.push(&record)
    }

    pub(super) fn metadata(&mut self) -> &mut Value {
        &mut self.metadata
    }

    // Вызывается только после последнего измеряемого кадра всех режимов.
    pub(super) fn finish(mut self) -> io::Result<Value> {
        let mut phases = Vec::new();
        let mut diagnostics = Vec::new();
        let mut work = Vec::new();
        let mut resources = Vec::new();
        for mut record in self.records.read()? {
            phases.push(record["phases"].take());
            diagnostics.push(record["diagnostics"].take());
            work.push(record["work"].take());
            resources.push(record["resources"].take());
        }
        let mut mode = report::mode(phases);
        mode["diagnostic_scopes"] = report::mode(diagnostics);
        mode["work_counters"] = json!({"samples": []});
        mode["work_counters"]["samples"] = Value::Array(work);
        mode["process_resources"] = json!({"samples": []});
        mode["process_resources"]["samples"] = Value::Array(resources);
        if let Some(fields) = self.metadata.as_object_mut() {
            for (key, value) in std::mem::take(fields) {
                mode[key] = value;
            }
        }
        Ok(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::super::report::Sample;
    #[cfg(feature = "allocation-audit")]
    use super::super::{args::Args, frames::Frames};
    use super::*;
    #[cfg(feature = "allocation-audit")]
    use gpui::frame_capture::CapturedFrame;
    #[cfg(feature = "allocation-audit")]
    use kamin_html::host::capture::Frame;

    fn measured(index: usize) -> Measured {
        Measured {
            phases: Sample::from([("build_elements", Some(index as f64)), ("parse", None)]),
            diagnostics: Sample::from([("cpu_draw_total", Some(1.))]),
        }
    }

    #[test]
    fn journal_preserves_raw_samples_skipped_scopes_statistics_and_metadata() {
        let mut history = History::new().unwrap();
        for index in 1..=10 {
            history
                .push(
                    measured(index),
                    json!({"index": index, "exact": 9_007_199_254_740_993u64}),
                    json!({"live": 42}),
                )
                .unwrap();
        }
        history.metadata()["matches_fresh"] = json!(true);
        let report = history.finish().unwrap();
        assert_eq!(report["samples_ms"].as_array().unwrap().len(), 10);
        assert_eq!(report["summary"]["build_elements"]["median_ms"], 5.5);
        assert_eq!(report["summary"]["build_elements"]["p90_ms"], 9.);
        assert!(report["summary"]["parse"].is_null());
        assert_eq!(report["work_counters"]["samples"][9]["index"], 10);
        assert_eq!(
            report["work_counters"]["samples"][0]["exact"].as_u64(),
            Some(9_007_199_254_740_993)
        );
        assert_eq!(report["process_resources"]["samples"][0]["live"], 42);
        assert_eq!(
            report["diagnostic_scopes"]["samples_ms"][0]["cpu_draw_total"],
            1.
        );
        assert_eq!(report["matches_fresh"], true);
    }

    #[cfg(feature = "allocation-audit")]
    #[test]
    fn repeated_relayout_does_not_retain_per_sample_heap() {
        let args = Args::parse(
            ["page.html", "--mode", "relayout", "--viewport", "2x2"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        let mut frames = Frames::new(&args).unwrap();
        let mut history = History::new().unwrap();
        let payload = "x".repeat(16_384);
        let mut record = |index: usize| {
            history
                .push(
                    measured(index),
                    json!({"payload": payload}),
                    json!({"phases": {"live": 123}}),
                )
                .unwrap();
            frames
                .record(
                    Frame {
                        seq: index as u64,
                        pixels: CapturedFrame {
                            width: 2,
                            height: 2,
                            bgra: vec![0; 16],
                            context: None,
                        },
                    },
                    "relayout",
                    index,
                    (1, 0),
                )
                .unwrap();
        };
        // Два RGB-эталона чередующихся viewport — допустимые ограниченные владельцы.
        record(1);
        record(2);
        let before = super::super::allocations::test_live::read();
        for index in 3..=1002 {
            record(index);
        }
        let after = super::super::allocations::test_live::read();
        assert!(
            after <= before + 1024,
            "retained sample heap grew by {} bytes",
            after - before
        );
        let report = history.finish().unwrap();
        assert_eq!(report["samples_ms"].as_array().unwrap().len(), 1002);
        assert_eq!(
            frames.report().unwrap()["document_frame_samples"]
                .as_array()
                .unwrap()
                .len(),
            1002
        );
    }
}
