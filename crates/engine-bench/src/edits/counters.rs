//! Edit work counters, captured outside effect sampling.
use super::Result;
use crate::view::View;
use serde_json::{Value, json};

/// The scene layer's process-wide work counters (`gpui::layer_counters`).
pub(super) fn layer_work() -> [u64; 6] {
    use gpui::layer_counters as c;
    use std::sync::atomic::Ordering::Relaxed;
    [
        c::RECORDS.load(Relaxed),
        c::COW_CLONES.load(Relaxed),
        c::VALUES_WRITTEN.load(Relaxed),
        c::BOXES_RERECORDED.load(Relaxed),
        c::PRIMITIVES_COPIED.load(Relaxed),
        c::PRESENTS_SKIPPED.load(Relaxed),
    ]
}

/// One sample's layer work and frame builds/replays of the document's
/// retained frame (a rebuilt document counts from zero again).
pub(super) fn layer_delta(before: ([u64; 6], (u64, u64)), after: ([u64; 6], (u64, u64))) -> Value {
    let d = |i: usize| after.0[i].wrapping_sub(before.0[i]);
    let since = |a: u64, b: u64| a.checked_sub(b).unwrap_or(a);
    json!({
        "layers_recorded": d(0),
        "cow_clones": d(1),
        "values_written": d(2),
        "boxes_rerecorded": d(3),
        "primitives_copied": d(4),
        "presents_skipped": d(5),
        "frame_builds": since(after.1.0, before.1.0),
        "frame_replays": since(after.1.1, before.1.1),
    })
}

/// Current document builds, replays and builds within `Window::draw`.
pub(super) fn counters(view: &View, _: &gpui::App) -> (u64, u64, u64) {
    let (builds, replays) = view.doc.retained_frames();
    (builds, replays, view.doc.builds_inside_draw())
}

/// Document work in an edit frame. Neither a build nor a replay means
/// counters did not start with the edit revision; it cannot count as zero work.
pub(super) fn frame_sample(
    index: usize,
    (builds, replays, inside): (u64, u64, u64),
) -> Result<Value> {
    if builds + replays == 0 {
        return Err(format!("edit frame {index} neither built nor replayed the document").into());
    }
    Ok(
        json!({"mode": "edit", "sample_index": index, "builds": builds, "replays": replays,
        "builds_inside_draw": inside}),
    )
}
