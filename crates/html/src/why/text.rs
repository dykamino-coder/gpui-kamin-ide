//! The people's form of the `why` journal: per frame its verdict, causes
//! with engine lines, refusals, recomputation by phase, cache misses by
//! reason and the owners whose memory changed.

use serde_json::Value;
use std::collections::BTreeMap;

mod activity;
mod frames;
mod memory;
mod raster;
mod roots;
mod window;
use frames::frames;
use raster::raster;
use std::fmt::Write;

/// Owners present in either report whose bytes changed, largest change first.
pub fn changes(before: &BTreeMap<String, u64>, now: &BTreeMap<String, u64>) -> Vec<(String, i64)> {
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
        roots::render(&mut s, &f["between"]["document_roots"], "root before");
        roots::render(&mut s, &f["document_roots"], "root");
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
        window::render(&mut s, f);
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
        memory::frame(&mut s, &f["memory"]);
    }
    for sample in out["memory_samples"].as_array().into_iter().flatten() {
        memory::sample(&mut s, sample);
    }
    activity::render(&mut s, out);
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
