//! Shared schema-1 frame recording for benchmarks and embedded hosts.

use crate::{Document, FrameCounters};
use gpui::{App, Window};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, string::String};

/// An explicitly enabled reader. No report, heap walk or clock read exists
/// in a host that has not created one.
pub struct Report {
    frames: Vec<Value>,
    owners: BTreeMap<String, u64>,
    status: Value,
    _reader: super::Subscription,
}

/// Journal and counter checkpoint before one frame/sample.
pub struct Watch {
    frames: FrameCounters,
    path: [u64; crate::paint_path::NAMES.len()],
    between: Value,
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

impl Report {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            owners: BTreeMap::new(),
            status: Value::Null,
            _reader: super::subscribe(),
        }
    }

    /// Start before the action that can build or replace the document tree.
    pub fn watch(&self, doc: &Document) -> Watch {
        Watch {
            frames: doc.frame_counters(),
            path: crate::paint_path::read(),
            between: super::to_json(&super::take()),
        }
    }

    /// A sample's additional fields (`mode`, `step`, `phases_ms`, `raster`,
    /// `error`, etc.) use the same protocol for every reader. `cold` counts
    /// the current document from zero, including work before the checkpoint.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        watch: Watch,
        doc: &Document,
        window: &Window,
        cx: &App,
        cold: bool,
        fields: Value,
    ) -> Result<usize, &'static str> {
        let recording = std::time::Instant::now();
        let counts = if cold {
            let (built, replayed) = doc.retained_frames();
            (built, replayed, doc.builds_inside_draw())
        } else {
            doc.frame_counters()
                .since(&watch.frames)
                .ok_or("document frame counter regressed")?
        };
        let memory_at = std::time::Instant::now();
        let mut memory = super::memory::owners(doc, window, cx, &mut self.owners);
        let memory_ms = memory_at.elapsed().as_secs_f64() * 1000.;
        if let Some(private) = fields["memory"]["process_private_bytes"].as_u64() {
            memory["process_private_bytes"] = json!(private);
            memory["outside_owners_bytes"] = memory["heap_bytes"]
                .as_u64()
                .map(|heap| json!(private as i64 - heap as i64))
                .unwrap_or(Value::Null);
        }
        let mut entry = super::to_json(&super::take());
        let after = crate::paint_path::read();
        let path: serde_json::Map<String, Value> = crate::paint_path::NAMES
            .iter()
            .enumerate()
            .filter(|&(i, name)| *name != "write_ns" && after[i] != watch.path[i])
            .map(|(i, name)| ((*name).into(), json!(after[i].wrapping_sub(watch.path[i]))))
            .collect();
        for (key, value) in fields.as_object().into_iter().flatten() {
            entry[key] = value.clone();
        }
        let common = json!({
            "frame": self.frames.len() + 1,
            "built": counts.0, "replayed": counts.1,
            "builds_inside_draw": counts.2,
            "verdict": if counts.0 > 0 { "built" } else if counts.1 > 0 { "replayed" } else { "none" },
            "paint_path": path, "memory": memory, "between": watch.between,
        });
        for (key, value) in common.as_object().into_iter().flatten() {
            entry[key] = value.clone();
        }
        entry["phases_ms"]["journal_memory"] = json!(memory_ms);
        entry["phases_ms"]["journal_record"] = json!(recording.elapsed().as_secs_f64() * 1000.);
        self.frames.push(entry);
        Ok(self.frames.len() - 1)
    }

    /// Enrich a drawn frame when its platform presentation is observed.
    #[cfg(feature = "why-profiler")]
    pub(super) fn annotate(&mut self, index: usize, fields: Value) {
        for (key, value) in fields.as_object().into_iter().flatten() {
            if key == "phases_ms" {
                for (name, time) in value.as_object().into_iter().flatten() {
                    self.frames[index][key][name] = time.clone();
                }
            } else {
                self.frames[index][key] = value.clone();
            }
        }
    }

    #[cfg(feature = "why-profiler")]
    pub(super) fn frames(&self) -> &[Value] {
        &self.frames
    }

    #[cfg(feature = "why-profiler")]
    pub(super) fn status(&mut self, status: Value) {
        self.status = status;
    }

    /// The same top-level document consumed by DevTools and engine-compare.
    pub fn to_json(&self) -> Value {
        let mut out = json!({"schema": super::SCHEMA, "frames": self.frames});
        if !self.status.is_null() {
            out["observations"] = self.status.clone();
        }
        out
    }

    /// Write both machine and human forms; callers choose when to do I/O,
    /// outside a measured frame.
    pub fn write(&self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(dir)?;
        let out = self.to_json();
        std::fs::write(dir.join("why.json"), serde_json::to_vec_pretty(&out)?)?;
        std::fs::write(dir.join("why.txt"), super::text::render(&out))?;
        Ok(())
    }
}

/// Raster protocol fields shared by captured benchmark and viewer frames.
pub fn raster(report: gpui::frame_capture::RasterReport) -> Value {
    json!({
        "plan": report.plan, "full_by": report.full_by, "shift_miss": report.shift_miss,
        "shift_compare": report.shift_compare, "delta": report.delta, "moved_px": report.moved_px,
        "rects_count": report.rects.len(), "raster_px": report.raster_px, "rects": report.rects,
        "gpu_wait_ms": report.gpu_wait.as_secs_f64() * 1e3,
    })
}
