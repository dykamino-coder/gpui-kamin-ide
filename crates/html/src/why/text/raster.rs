//! Raster explanation.
use serde_json::Value;
use std::fmt::Write;

/// The raster line of a captured frame and its GPU wait on a line of its own:
/// the wait is the GPU's time, not CPU work of the frame.
pub(super) fn raster(s: &mut String, r: &Value) {
    if r.is_null() {
        return;
    }
    let full_by = r["full_by"]
        .as_str()
        .map(|w| format!(" by {w}"))
        .unwrap_or_default();
    let full_by = full_by
        + &r["shift_miss"]
            .as_str()
            .map(|w| format!(" (not a move: {w})"))
            .unwrap_or_default();
    let delta = r["delta"]
        .as_array()
        .map(|d| format!(" delta {},{} moved_px {}", d[0], d[1], r["moved_px"]))
        .unwrap_or_default();
    let rects: Vec<String> = r["rects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| format!("{},{} {}x{}", q[0], q[1], q[2], q[3]))
        .collect();
    let _ = writeln!(
        s,
        "  raster {}{full_by}{delta} rects {} raster_px {} [{}]",
        r["plan"].as_str().unwrap_or("?"),
        r["rects_count"],
        r["raster_px"],
        rects.join(" | ")
    );
    if let Some(c) = r["shift_compare"].as_str() {
        let _ = writeln!(s, "  compared {c}");
    }
    let _ = write!(
        s,
        "  gpu_wait {:.3} ms",
        r["gpu_wait_ms"].as_f64().unwrap_or(0.0)
    );
    for (field, label) in [("submit_ms", "submit"), ("capture_copy_ms", "capture_copy")] {
        if let Some(ms) = r[field].as_f64() {
            let _ = write!(s, " {label} {ms:.3} ms");
        }
    }
    let _ = writeln!(s);
}
