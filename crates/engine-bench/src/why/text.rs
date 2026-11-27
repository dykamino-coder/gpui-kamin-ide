//! The people's form of the `why` journal: per frame its verdict, causes
//! with engine lines, refusals, recomputation by phase, cache misses by
//! reason and the owners whose memory changed.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Owners present in either report whose bytes changed, largest change first.
pub(super) fn changes(
    before: &BTreeMap<String, u64>,
    now: &BTreeMap<String, u64>,
) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = before
        .keys()
        .chain(now.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|k| {
            let get = |m: &BTreeMap<String, u64>| m.get(k).copied().unwrap_or(0) as i64;
            (k.clone(), get(now) - get(before))
        })
        .filter(|(_, d)| *d != 0)
        .collect();
    out.sort_by_key(|(k, d)| (std::cmp::Reverse(d.unsigned_abs()), k.clone()));
    out
}

const SHOWN_OWNERS: usize = 8;

/// The raster line of a captured frame and its GPU wait on a line of its own:
/// the wait is the GPU's time, not CPU work of the frame.
fn raster(s: &mut String, r: &Value) {
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
    let _ = writeln!(
        s,
        "  gpu_wait {:.3} ms",
        r["gpu_wait_ms"].as_f64().unwrap_or(0.0)
    );
}

/// The text form of `why.json`.
pub(super) fn render(out: &Value) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "why journal, schema {}", out["schema"]);
    for f in out["frames"].as_array().into_iter().flatten() {
        let _ = writeln!(
            s,
            "\n#{} {}/{}: {} (built {}, replayed {})",
            f["frame"],
            text(&f["mode"]),
            text(&f["step"]),
            text(&f["verdict"]),
            f["built"],
            f["replayed"]
        );
        // The same decision taken again in the frame (the step and the draw both check) reads once.
        let mut causes: Vec<(&Value, usize)> = Vec::new();
        for c in f["causes"].as_array().into_iter().flatten() {
            match causes.iter_mut().find(|(seen, _)| *seen == c) {
                Some((_, n)) => *n += 1,
                None => causes.push((c, 1)),
            }
        }
        for (c, n) in causes {
            line(
                &mut s,
                &if n > 1 {
                    format!("cause x{n}")
                } else {
                    "cause".into()
                },
                c,
            );
        }
        if let Some(error) = f["error"].as_str() {
            let _ = writeln!(s, "  error  {error}");
        }
        frames(&mut s, "  frame ", &f["frames"], &f["frames_dropped"]);
        frames(
            &mut s,
            "  before",
            &f["between"]["frames"],
            &f["between"]["frames_dropped"],
        );
        if f["causes_dropped"].as_u64().unwrap_or(0) > 0 {
            let _ = writeln!(s, "  cause  ... {} more not kept", f["causes_dropped"]);
        }
        pairs(&mut s, "path ", &f["paint_path"]);
        super::paint_work::render(&mut s, &f["paint_work"]);
        super::layout_work::render(&mut s, &f["layout_work"]);
        super::element_kinds::render(&mut s, &f["request_layout_kinds"]);
        super::layout_inputs::render(&mut s, &f["layout_inputs"]);
        let phases: Vec<String> = f["phases_ms"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| {
                v.as_f64()
                    .filter(|ms| *ms > 0.0)
                    .map(|ms| format!("{k}={ms:.3}"))
            })
            .collect();
        if !phases.is_empty() {
            let _ = writeln!(s, "  phase  {}", phases.join(" "));
        }
        raster(&mut s, &f["raster"]);
        if let Some(line) = f["gpu_batches"].as_str() {
            let _ = writeln!(s, "  {line}");
        }
        for (name, c) in f["caches"].as_object().into_iter().flatten() {
            let misses: Vec<String> = c["misses"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            let _ = writeln!(
                s,
                "  cache  {name}: hits={} misses: {}",
                c["hits"],
                misses.join(" ")
            );
            for e in c["examples"].as_array().into_iter().flatten() {
                line(&mut s, "  miss", e);
            }
        }
        let m = &f["memory"];
        let _ = writeln!(
            s,
            "  memory heap owners {} bytes, gpu {} bytes, process private {} bytes, outside the owners' report {} bytes",
            m["heap_bytes"], m["gpu_bytes"], m["process_private_bytes"], m["outside_owners_bytes"]
        );
        for c in m["changed_bytes"]
            .as_array()
            .into_iter()
            .flatten()
            .take(SHOWN_OWNERS)
        {
            let _ = writeln!(
                s,
                "  memory {:+} {}",
                c[1].as_i64().unwrap_or(0),
                text(&c[0])
            );
        }
    }
    s
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn line(s: &mut String, label: &str, c: &Value) {
    let detail = text(&c["detail"]);
    let sep = if detail.is_empty() { "" } else { ": " };
    let _ = writeln!(
        s,
        "  {label}  {} at {}{sep}{detail}",
        text(&c["code"]),
        text(&c["site"])
    );
}

/// The frame events in order, repeats of one event in a row as `xN`, and a
/// summary of draws and renders with the requests after the first render
/// (who asked for a frame beyond the expected one).
fn frames(s: &mut String, label: &str, events: &Value, dropped: &Value) {
    let all = events.as_array().map_or(&[][..], Vec::as_slice);
    if all.is_empty() {
        return;
    }
    let count = |kind: &str| all.iter().filter(|e| e["kind"] == kind).count();
    let first_render = all.iter().position(|e| e["kind"] == "render");
    let late: Vec<String> = first_render
        .map(|at| &all[at + 1..])
        .unwrap_or_default()
        .iter()
        .filter(|e| {
            e["kind"] != "draw"
                && e["kind"] != "render"
                && !text(&e["kind"]).starts_with("scroll_node.")
        })
        .map(|e| format!("{} at {}", text(&e["kind"]), text(&e["site"])))
        .collect();
    let _ = write!(
        s,
        "{label} draws={} renders={}",
        count("draw"),
        count("render")
    );
    if !late.is_empty() {
        let _ = write!(s, "; requested after the first render: {}", late.join(", "));
    }
    let _ = writeln!(s);
    let mut at = 0;
    while at < all.len() {
        let run = all[at..].iter().take_while(|e| *e == &all[at]).count();
        let times = if run > 1 {
            format!(" x{run}")
        } else {
            String::new()
        };
        let detail = text(&all[at]["detail"]);
        let sep = if detail.is_empty() { "" } else { ": " };
        let _ = writeln!(
            s,
            "{label}   {}{times} at {}{sep}{detail}",
            text(&all[at]["kind"]),
            text(&all[at]["site"])
        );
        at += run;
    }
    if dropped.as_u64().unwrap_or(0) > 0 {
        let _ = writeln!(s, "{label}   ... {dropped} more not kept");
    }
}

fn pairs(s: &mut String, label: &str, v: &Value) {
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

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
