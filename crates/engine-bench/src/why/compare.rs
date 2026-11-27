//! `--why-compare A B`: what changed between two `--why` runs (two builds,
//! two pages states): verdicts, causes by code and line, cache misses by
//! reason, frame requests by kind and line, and memory owners. Totals over
//! all frames first (what a regression's author looks for), then per frame
//! the frames whose verdict, causes or requests differ.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;

type Counts = BTreeMap<String, i64>;

/// Owners whose bytes changed shown per comparison.
const SHOWN_OWNERS: usize = 12;

/// Read `why.json` of each run (a file or its directory) and compare.
pub(in super::super) fn run(
    a: &std::path::Path,
    b: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let read = |p: &std::path::Path| -> Result<Value, Box<dyn std::error::Error>> {
        let file = if p.is_dir() {
            p.join("why.json")
        } else {
            p.to_path_buf()
        };
        Ok(serde_json::from_slice(
            &std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?,
        )?)
    };
    let (a, b) = (read(a)?, read(b)?);
    if a["schema"] != b["schema"] {
        return Err(format!("schemas differ: {} vs {}", a["schema"], b["schema"]).into());
    }
    Ok(compare(&a, &b))
}

fn frames(run: &Value) -> &[Value] {
    run["frames"].as_array().map_or(&[], Vec::as_slice)
}

/// Causes as `code at site`, cache misses as `cache/reason`, frame events as
/// `kind at site`, verdicts as `verdict`.
fn counts(frames: &[Value]) -> [Counts; 4] {
    let mut out: [Counts; 4] = Default::default();
    for f in frames {
        for c in f["causes"].as_array().into_iter().flatten() {
            *out[0]
                .entry(format!("{} at {}", text(&c["code"]), text(&c["site"])))
                .or_default() += 1;
        }
        for (name, c) in f["caches"].as_object().into_iter().flatten() {
            for (reason, n) in c["misses"].as_object().into_iter().flatten() {
                *out[1].entry(format!("{name}/{reason}")).or_default() += n.as_i64().unwrap_or(0);
            }
        }
        for e in f["frames"].as_array().into_iter().flatten() {
            *out[2]
                .entry(format!("{} at {}", text(&e["kind"]), text(&e["site"])))
                .or_default() += 1;
        }
        *out[3].entry(text(&f["verdict"]).to_string()).or_default() += 1;
    }
    out
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// Lines `name: a -> b (+d)` of the keys whose counts differ.
fn diff(s: &mut String, label: &str, a: &Counts, b: &Counts) {
    let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
    for k in keys {
        let (x, y) = (
            a.get(k).copied().unwrap_or(0),
            b.get(k).copied().unwrap_or(0),
        );
        if x != y {
            let _ = writeln!(s, "  {label} {k}: {x} -> {y} ({:+})", y - x);
        }
    }
}

fn owners(f: &Value) -> BTreeMap<String, u64> {
    f["owners"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
        .collect()
}

fn memory(run: &Value) -> Option<&Value> {
    run["memory_samples"]
        .as_array()
        .and_then(|samples| samples.last())
        .or_else(|| frames(run).last().map(|frame| &frame["memory"]))
}

/// The text of the comparison of runs `a` and `b`.
pub(super) fn compare(a: &Value, b: &Value) -> String {
    let (fa, fb) = (frames(a), frames(b));
    let mut s = String::new();
    let _ = writeln!(
        s,
        "why compare: A {} frames, B {} frames",
        fa.len(),
        fb.len()
    );
    let (ca, cb) = (counts(fa), counts(fb));
    let _ = writeln!(s, "\ntotals (A -> B)");
    for (at, label) in ["verdict", "cause", "miss", "request"]
        .into_iter()
        .enumerate()
    {
        let at = [3, 0, 1, 2][at];
        diff(&mut s, label, &ca[at], &cb[at]);
    }
    // Explicit end-of-series snapshots; older reports kept owners in frames.
    if let (Some(la), Some(lb)) = (memory(a), memory(b)) {
        let changed = kamin_html::why::owner_changes(&owners(la), &owners(lb));
        for (owner, d) in changed.iter().take(SHOWN_OWNERS) {
            let _ = writeln!(s, "  memory {d:+} {owner}");
        }
        for key in [
            "heap_bytes",
            "gpu_bytes",
            "process_private_bytes",
            "outside_owners_bytes",
        ] {
            let (x, y) = (la[key].as_i64(), lb[key].as_i64());
            if let (Some(x), Some(y)) = (x, y)
                && x != y
            {
                let _ = writeln!(s, "  memory {key}: {x} -> {y} ({:+})", y - x);
            }
        }
    }
    for (at, (x, y)) in fa.iter().zip(fb).enumerate() {
        let (cx, cy) = (
            counts(std::slice::from_ref(x)),
            counts(std::slice::from_ref(y)),
        );
        let verdict = (text(&x["verdict"]), text(&y["verdict"]));
        if verdict.0 == verdict.1 && cx[0] == cy[0] && cx[2] == cy[2] && x["error"] == y["error"] {
            continue;
        }
        let _ = writeln!(
            s,
            "\n#{} {}/{}: {} -> {}",
            at + 1,
            text(&x["mode"]),
            text(&x["step"]),
            verdict.0,
            verdict.1
        );
        if x["error"] != y["error"] {
            let _ = writeln!(s, "  error {} -> {}", x["error"], y["error"]);
        }
        diff(&mut s, "cause", &cx[0], &cy[0]);
        diff(&mut s, "request", &cx[2], &cy[2]);
    }
    s
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn compares_explicit_final_owner_samples_instead_of_frame_references() {
        let run = |bytes| {
            json!({"schema": 1,
                "frames": [{"memory": {"sample_id": 1, "sampled": false}}],
                "memory_samples": [{"owners": {"document": 1}},
                    {"reason": "series_end", "owners": {"document": bytes}, "heap_bytes": bytes,
                        "process_private_bytes": bytes * 2, "outside_owners_bytes": bytes}]
            })
        };
        let text = super::compare(&run(10), &run(50));
        assert!(text.contains("memory +40 document"));
        assert!(text.contains("memory heap_bytes: 10 -> 50 (+40)"));
        assert!(text.contains("memory process_private_bytes: 20 -> 100 (+80)"));
        assert!(text.contains("memory outside_owners_bytes: 10 -> 50 (+40)"));
    }

    #[test]
    fn names_what_changed_between_two_runs() {
        let frame = |verdict: &str, code: &str, evicted: u64, extra: bool, bytes: u64| {
            let mut events = vec![
                json!({"kind": "draw", "site": "w.rs:1"}),
                json!({"kind": "render", "site": "v.rs:2"}),
            ];
            if extra {
                events.push(json!({"kind": "notify", "site": "x.rs:3"}));
            }
            json!({"mode": "relayout", "step": "warm", "verdict": verdict,
                "causes": [{"code": code, "site": "r.rs:9", "detail": ""}],
                "caches": {"tiles": {"misses": {"evicted": evicted}}},
                "frames": events, "error": null,
                "memory": {"owners": {"style/computed": bytes}, "heap_bytes": bytes}})
        };
        let a = json!({"schema": 1, "frames": [frame("replayed", "frame_miss.key", 0, false, 10)]});
        let b = json!({"schema": 1, "frames": [frame("built", "frame_miss.stale", 3, true, 50)]});
        let s = super::compare(&a, &b);
        assert!(s.contains("  verdict built: 0 -> 1 (+1)"), "{s}");
        assert!(s.contains("  cause frame_miss.key at r.rs:9: 1 -> 0 (-1)"));
        assert!(s.contains("  miss tiles/evicted: 0 -> 3 (+3)"));
        assert!(s.contains("  request notify at x.rs:3: 0 -> 1 (+1)"));
        assert!(s.contains("  memory +40 style/computed"));
        assert!(s.contains("  memory heap_bytes: 10 -> 50 (+40)"));
        assert!(s.contains("#1 relayout/warm: replayed -> built"));
        assert_eq!(
            super::compare(&a, &a)
                .lines()
                .filter(|l| l.starts_with("  "))
                .count(),
            0
        );
    }
}
