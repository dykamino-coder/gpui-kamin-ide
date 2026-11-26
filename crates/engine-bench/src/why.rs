//! `--why DIR`: the first reader of the DevTools `why` journal
//! (`kamin_html::why`). Each sampled frame gets one entry: built or replayed,
//! the journal's causes with their engine lines, the requests of frames and
//! the draws and renders they caused, the paint-path refusals, the phase
//! times, the cache lookups with the reasons of their misses and the memory
//! by owner with what grew. A failed sample keeps its entry with the error.
//! `why.json` (schema `kamin_html::why::SCHEMA`) is for the browser panel
//! and `tools/engine-compare`; `why.txt` for people; `--why-compare A B`
//! reads two of them (`compare`).

pub(super) mod compare;
mod paint_work;
mod text;

use super::run::Step;
use super::view::View;
use gpui::{AsyncApp, WindowHandle};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

thread_local! {
    static FRAMES: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static OWNERS: RefCell<BTreeMap<String, u64>> = const { RefCell::new(BTreeMap::new()) };
    /// The reader's subscription, for the whole run.
    static READER: RefCell<Option<kamin_html::why::Subscription>> = const { RefCell::new(None) };
}

/// Counters before one sample.
pub(super) struct Watch {
    frames: kamin_html::FrameCounters,
    path: [u64; kamin_html::paint_path::NAMES.len()],
    /// What was recorded between the previous sample and this one (a frame
    /// that came late belongs to neither).
    between: Value,
}

impl Watch {
    /// `None` without `--why`; subscribes at the first sample.
    pub(super) fn begin(
        args: &super::args::Args,
        cx: &AsyncApp,
        window: WindowHandle<View>,
    ) -> Result<Option<Self>> {
        if args.why.is_none() {
            return Ok(None);
        }
        READER.with(|r| {
            r.borrow_mut()
                .get_or_insert_with(kamin_html::why::subscribe);
        });
        let between = kamin_html::why::to_json(&kamin_html::why::take());
        let frames = window.read_with(cx, |view, _| view.doc.frame_counters())?;
        let path = kamin_html::paint_path::read();
        Ok(Some(Self {
            frames,
            path,
            between,
        }))
    }

    /// The entry of the sample that just ended (or failed with `error`),
    /// written out with the others; `raster`: what its captured frame
    /// rasterized and the capture's wait for the GPU.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn end(
        self,
        args: &super::args::Args,
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        step: Step<'_>,
        measurements: Option<(&super::report::Sample, &super::work::Sample)>,
        raster: Option<gpui::frame_capture::RasterReport>,
        error: Option<String>,
    ) -> Result<()> {
        let (phases, work) = measurements.unzip();
        let (frames, mut memory) = window.update(cx, |view, window, cx| {
            (
                (view.doc.frame_counters(), frames(&view.doc)),
                owners(&view.doc, window, cx),
            )
        })?;
        // What the process holds beyond the named heap owners (allocator, driver, unreported).
        let private = super::process::Snapshot::read()?.commit;
        let heap = memory["heap_bytes"].as_u64().unwrap_or(0);
        memory["process_private_bytes"] = json!(private);
        memory["outside_owners_bytes"] = json!(private as i64 - heap as i64);
        let (checkpoint, totals) = frames;
        let (built, replayed, _) = if matches!(step, Step::Cold(_)) {
            totals
        } else {
            checkpoint
                .since(&self.frames)
                .ok_or("document frame counter regressed")?
        };
        let after = kamin_html::paint_path::read();
        let path: serde_json::Map<String, Value> = kamin_html::paint_path::NAMES
            .iter()
            .enumerate()
            .filter(|&(i, name)| *name != "write_ns" && after[i] != self.path[i])
            .map(|(i, name)| ((*name).into(), json!(after[i].wrapping_sub(self.path[i]))))
            .collect();
        let mut entry = kamin_html::why::to_json(&kamin_html::why::take());
        // Primitives and painted area by batch kind since the last sample (`[gpu]`,
        // with GPU time per kind under KAMIN_DIAG_GPU_TIMING=1).
        let gpu_batches = cx.update(|cx| cx.instrumentation().batches.take_line());
        let index = FRAMES.with(|f| f.borrow().len()) + 1;
        let fields = json!({
            "frame": index,
            "mode": format!("{:?}", args.mode).to_lowercase(),
            "step": match step { Step::Cold(_) => "cold", Step::Warm => "warm", Step::Edit(_) => "edit" },
            "built": built, "replayed": replayed,
            "verdict": if built > 0 { "built" } else if replayed > 0 { "replayed" } else { "none" },
            "error": error,
            "paint_path": path,
            "phases_ms": phases,
            "paint_work": work.map(super::work::Sample::paint_profile),
            "raster": raster.map(|r| json!({
                "plan": r.plan, "full_by": r.full_by, "shift_miss": r.shift_miss, "shift_compare": r.shift_compare, "delta": r.delta, "moved_px": r.moved_px,
                "rects_count": r.rects.len(), "raster_px": r.raster_px, "rects": r.rects,
                "gpu_wait_ms": r.gpu_wait.as_secs_f64() * 1e3,
            })),
            "gpu_batches": gpu_batches,
            "memory": memory,
            "between": self.between,
        });
        for (key, value) in fields.as_object().into_iter().flatten() {
            entry[key] = value.clone();
        }
        FRAMES.with(|f| f.borrow_mut().push(entry));
        write(args.why.as_deref().ok_or("--why without a directory")?)
    }
}

/// Builds, replays and builds inside the draw of the document's frame.
fn frames(doc: &kamin_html::Document) -> (u64, u64, u64) {
    let (built, replayed) = doc.retained_frames();
    (built, replayed, doc.builds_inside_draw())
}

/// Rust heap of the document and the window by owner, the process caches and
/// the window's GPU storage (`gpu/`, not heap), and what changed since the
/// previous entry (the holder of growth).
fn owners(doc: &kamin_html::Document, window: &gpui::Window, cx: &gpui::App) -> Value {
    let mut ops = kamin_html::heap::Ops::default();
    doc.heap_report(&mut ops);
    window.heap_report(cx, &mut |path, pointer, bytes| {
        if pointer.is_null() || ops.once(pointer) {
            ops.at(path, |ops| ops.block(pointer, bytes));
        }
    });
    let mut now = ops.report().entries().clone();
    // Owners outside the document: the process-wide caches and the window's GPU storage.
    for (name, bytes) in kamin_html::why::caches_held() {
        now.insert(format!("process/cache/{name}"), bytes as u64);
    }
    window.gpu_memory(&mut |path, bytes, _used| {
        *now.entry(format!("gpu/{path}")).or_default() += bytes;
    });
    let grew = OWNERS.with(|o| {
        let before = std::mem::replace(&mut *o.borrow_mut(), now.clone());
        text::changes(&before, &now)
    });
    let gpu: u64 = now
        .iter()
        .filter(|(k, _)| k.starts_with("gpu/"))
        .map(|(_, v)| v)
        .sum();
    let heap = now.values().sum::<u64>() - gpu;
    json!({"heap_bytes": heap, "gpu_bytes": gpu, "owners": now, "changed_bytes": grew})
}

fn write(dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let frames = FRAMES.with(|f| f.borrow().clone());
    let out = json!({"schema": kamin_html::why::SCHEMA, "frames": frames});
    std::fs::write(dir.join("why.json"), serde_json::to_vec_pretty(&out)?)?;
    std::fs::write(dir.join("why.txt"), text::render(&out))?;
    Ok(())
}
