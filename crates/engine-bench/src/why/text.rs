//! The people's form of the `why` journal: per frame its verdict, causes
//! with engine lines, refusals, recomputation by phase, cache misses by
//! reason and the owners whose memory changed.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Owners present in either report whose bytes changed, largest change first.
pub(super) fn changes(before: &BTreeMap<String, u64>, now: &BTreeMap<String, u64>) -> Vec<(String, i64)> {
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

/// The text form of `why.json`.
pub(super) fn render(out: &Value) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "why journal, schema {}", out["schema"]);
    for f in out["frames"].as_array().into_iter().flatten() {
        let _ = writeln!(
            s,
            "\n#{} {}/{}: {} (built {}, replayed {})",
            f["frame"], text(&f["mode"]), text(&f["step"]), text(&f["verdict"]), f["built"], f["replayed"]
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
            line(&mut s, &if n > 1 { format!("cause x{n}") } else { "cause".into() }, c);
        }
        if f["causes_dropped"].as_u64().unwrap_or(0) > 0 {
            let _ = writeln!(s, "  cause  ... {} more not kept", f["causes_dropped"]);
        }
        pairs(&mut s, "path ", &f["paint_path"]);
        let phases: Vec<String> = f["phases_ms"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| v.as_f64().filter(|ms| *ms > 0.0).map(|ms| format!("{k}={ms:.3}")))
            .collect();
        if !phases.is_empty() {
            let _ = writeln!(s, "  phase  {}", phases.join(" "));
        }
        for (name, c) in f["caches"].as_object().into_iter().flatten() {
            let misses: Vec<String> = c["misses"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            let _ = writeln!(s, "  cache  {name}: hits={} misses: {}", c["hits"], misses.join(" "));
            for e in c["examples"].as_array().into_iter().flatten() {
                line(&mut s, "  miss", e);
            }
        }
        let m = &f["memory"];
        let _ = writeln!(
            s,
            "  memory named owners {} bytes, process private {} bytes",
            m["total_bytes"], m["process_private_bytes"]
        );
        for c in m["changed_bytes"].as_array().into_iter().flatten().take(SHOWN_OWNERS) {
            let _ = writeln!(s, "  memory {:+} {}", c[1].as_i64().unwrap_or(0), text(&c[0]));
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
    let _ = writeln!(s, "  {label}  {} at {}{sep}{detail}", text(&c["code"]), text(&c["site"]));
}

fn pairs(s: &mut String, label: &str, v: &Value) {
    let all: Vec<String> = v.as_object().into_iter().flatten().map(|(k, n)| format!("{k}={n}")).collect();
    if !all.is_empty() {
        let _ = writeln!(s, "  {label} {}", all.join(" "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn growth_is_ordered_by_size_and_names_new_and_gone_owners() {
        let before = BTreeMap::from([("a".to_string(), 10), ("b".to_string(), 5)]);
        let now = BTreeMap::from([("a".to_string(), 11), ("c".to_string(), 100)]);
        assert_eq!(changes(&before, &now), [("c".into(), 100), ("b".into(), -5), ("a".into(), 1)]);
    }

    #[test]
    fn a_frame_reads_as_verdict_cause_line_and_cache_reason() {
        let out = json!({"schema": 1, "frames": [{
            "frame": 2, "mode": "scroll", "step": "warm", "verdict": "built", "built": 1, "replayed": 0,
            "causes": [{"code": "frame_miss.scroll_deferreddraws", "site": "vendor/gpui/src/window/scroll_replay.rs:96",
                "detail": "1 deferred draws in the record, made at: crates/html/src/paint/stacking.rs:121"},
                {"code": "frame_miss.key", "site": "a.rs:1", "detail": ""},
                {"code": "frame_miss.key", "site": "a.rs:1", "detail": ""}],
            "causes_dropped": 0, "paint_path": {"refused_no_layer": 1}, "phases_ms": {"layout": 2.5, "paint": null},
            "caches": {"gradient_tiles": {"hits": 3, "misses": {"evicted": 2}, "examples": []}},
            "memory": {"total_bytes": 9, "changed_bytes": [["style/computed", 4]]},
        }]});
        let s = render(&out);
        assert!(s.contains("#2 scroll/warm: built (built 1, replayed 0)"));
        assert!(s.contains("frame_miss.scroll_deferreddraws at vendor/gpui/src/window/scroll_replay.rs:96: 1 deferred"));
        assert!(s.contains("cause x2  frame_miss.key at a.rs:1\n"));
        assert!(s.contains("path  refused_no_layer=1"));
        assert!(s.contains("phase  layout=2.500"));
        assert!(s.contains("gradient_tiles: hits=3 misses: evicted=2"));
        assert!(s.contains("memory +4 style/computed"));
    }
}
