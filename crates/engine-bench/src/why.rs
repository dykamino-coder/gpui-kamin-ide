//! `--why DIR`: the first reader of the DevTools `why` journal
//! (`kamin_html::why`). Each sampled frame gets one entry: built or replayed,
//! the causes with their engine lines, the paint-path refusals, the phase
//! times, the cache lookups with the reasons of their misses and the memory
//! by owner with what grew. `why.json` (schema `kamin_html::why::SCHEMA`) is
//! for the browser panel and `tools/engine-compare`; `why.txt` for people.

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
}

/// Counters before one sample.
pub(super) struct Watch {
    frames: (u64, u64, u64),
    path: [u64; kamin_html::paint_path::NAMES.len()],
}

impl Watch {
    /// `None` without `--why`; turns the journal on at the first sample.
    pub(super) fn begin(
        args: &super::args::Args,
        cx: &AsyncApp,
        window: WindowHandle<View>,
    ) -> Result<Option<Self>> {
        if args.why.is_none() {
            return Ok(None);
        }
        kamin_html::why::set_enabled(true);
        let frames = window.read_with(cx, |view, _| frames(&view.doc))?;
        let path = kamin_html::paint_path::read();
        Ok(Some(Self { frames, path }))
    }

    /// The entry of the sample that just ended, written out with the others.
    pub(super) fn end(
        self,
        args: &super::args::Args,
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        step: Step<'_>,
        phases: &super::report::Sample,
    ) -> Result<()> {
        let (frames, mut memory) = window.update(cx, |view, window, cx| {
            (frames(&view.doc), owners(&view.doc, window, cx))
        })?;
        // What the process holds beyond the named owners (allocator, driver, unreported).
        let private = super::process::Snapshot::read()?.commit;
        memory["process_private_bytes"] = json!(private);
        // A new document (cold) or revision (edit) counts from zero: its counters are the frame's.
        let fresh = !matches!(step, Step::Warm);
        let since = |after: u64, before: u64| if fresh { after } else { after.saturating_sub(before) };
        // Builds by the document step and inside the window's draw.
        let built = since(frames.0, self.frames.0) + since(frames.2, self.frames.2);
        let replayed = since(frames.1, self.frames.1);
        let after = kamin_html::paint_path::read();
        let path: serde_json::Map<String, Value> = kamin_html::paint_path::NAMES
            .iter()
            .enumerate()
            .filter(|&(i, name)| *name != "write_ns" && after[i] != self.path[i])
            .map(|(i, name)| ((*name).into(), json!(after[i].wrapping_sub(self.path[i]))))
            .collect();
        let journal = kamin_html::why::take();
        let index = FRAMES.with(|f| f.borrow().len()) + 1;
        let entry = json!({
            "frame": index,
            "mode": format!("{:?}", args.mode).to_lowercase(),
            "step": match step { Step::Cold(_) => "cold", Step::Warm => "warm", Step::Edit(_) => "edit" },
            "built": built, "replayed": replayed,
            "verdict": if built > 0 { "built" } else if replayed > 0 { "replayed" } else { "none" },
            "causes": journal.causes.iter().map(cause).collect::<Vec<_>>(),
            "causes_dropped": journal.dropped,
            "paint_path": path,
            "phases_ms": phases,
            "caches": journal.caches.iter().map(|(name, c)| ((*name).to_string(), json!({
                "hits": c.hits, "misses": c.misses,
                "examples": c.examples.iter().map(cause).collect::<Vec<_>>(),
            }))).collect::<serde_json::Map<_, _>>(),
            "memory": memory,
        });
        FRAMES.with(|f| f.borrow_mut().push(entry));
        write(args.why.as_deref().ok_or("--why without a directory")?)
    }
}

/// Builds, replays and builds inside the draw of the document's frame.
fn frames(doc: &kamin_html::Document) -> (u64, u64, u64) {
    let (built, replayed) = doc.retained_frames();
    (built, replayed, doc.builds_inside_draw())
}

fn cause(c: &kamin_html::why::Cause) -> Value {
    json!({"code": c.code, "site": c.site(), "detail": c.detail})
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
    json!({"total_bytes": now.values().sum::<u64>(), "owners": now, "changed_bytes": grew})
}

fn write(dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let frames = FRAMES.with(|f| f.borrow().clone());
    let out = json!({"schema": kamin_html::why::SCHEMA, "frames": frames});
    std::fs::write(dir.join("why.json"), serde_json::to_vec_pretty(&out)?)?;
    std::fs::write(dir.join("why.txt"), text::render(&out))?;
    Ok(())
}
