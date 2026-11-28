//! Frame work: raster, scrolling animation and named counters.

use serde_json::Value;
use std::fmt::Write;

/// The raster line of a captured frame, then its GPU wait, command submission
/// and the bench's own copy of the frame to memory (not engine work).
pub(super) fn raster(s: &mut String, r: &Value) {
    if r.is_null() {
        return;
    }
    let tag = |k: &str, a: &str, b: &str| {
        r[k].as_str()
            .map_or(String::new(), |w| format!("{a}{w}{b}"))
    };
    let full_by = tag("full_by", " by ", "") + &tag("shift_miss", " (not a move: ", ")");
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
    let _ = writeln!(
        s,
        "  gpu_wait {:.3} ms submit {:.3} ms capture_copy {:.3} ms",
        r["gpu_wait_ms"].as_f64().unwrap_or(0.0),
        r["submit_ms"].as_f64().unwrap_or(0.0),
        r["capture_copy_ms"].as_f64().unwrap_or(0.0)
    );
}

pub(super) fn pairs(s: &mut String, label: &str, v: &Value) {
    let all: Vec<String> = v
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, n)| format!("{k}={n}"))
        .collect();
    if !all.is_empty() {
        let _ = writeln!(s, "  {label} {}", all.join(" "));
    }
}

pub(super) fn animation(s: &mut String, current: &Value, before: &Value) {
    let count = |key: &str| current[key].as_u64().unwrap_or(0) + before[key].as_u64().unwrap_or(0);
    if count("frames") > 0 {
        let _ = writeln!(
            s,
            "  scroll animation frames {} / builds {}",
            count("frames"),
            count("builds")
        );
    }
}
