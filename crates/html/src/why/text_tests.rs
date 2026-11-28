//! Frame explanation text tests.

use super::*;
use serde_json::json;

#[test]
fn explicit_memory_samples_are_not_reported_as_current_frame_owners() {
    let out = json!({"schema": 1, "frames": [{
        "frame": 1, "memory": {"sample_id": 1, "sampled": false}
    }], "memory_samples": [{
        "sample_id": 1, "reason": "open", "after_frame": 0, "duration_ms": 64.5,
        "heap_bytes": 100, "gpu_bytes": 0, "changed_bytes": [["document", 100]]
    }]});
    let rendered = render(&out);
    let (frame, snapshot) = rendered.split_once("memory snapshot").unwrap();
    assert!(frame.contains("memory not sampled; latest snapshot 1"));
    assert!(!frame.contains("heap owners"));
    assert!(snapshot.contains("1: open after frame 0, 64.500 ms"));
    assert!(snapshot.contains("heap owners 100 bytes"));
    assert!(snapshot.contains("memory +100 document"));
}

#[test]
fn growth_is_ordered_by_size_and_names_new_and_gone_owners() {
    let before = BTreeMap::from([("a".to_string(), 10), ("b".to_string(), 5)]);
    let now = BTreeMap::from([("a".to_string(), 11), ("c".to_string(), 100)]);
    assert_eq!(
        changes(&before, &now),
        [("c".into(), 100), ("b".into(), -5), ("a".into(), 1)]
    );
}

#[test]
fn a_frame_reads_as_verdict_cause_line_and_cache_reason() {
    let out = json!({"schema": 1, "frames": [{
        "frame": 2, "mode": "scroll", "step": "warm", "verdict": "built", "built": 1, "replayed": 0,
        "causes": [{"code": "frame_miss.scroll_deferreddraws", "site": "vendor/gpui/src/window/scroll_replay.rs:96",
            "detail": "1 deferred draws in the record, made at: crates/html/src/paint/stacking.rs:121"},
            {"code": "frame_miss.key", "site": "a.rs:1", "detail": ""},
            {"code": "frame_miss.key", "site": "a.rs:1", "detail": ""}],
        "scroll_animation": {"frames": 3, "builds": 0},
        "causes_dropped": 0, "paint_path": {"refused_no_layer": 1}, "phases_ms": {"layout": 2.5, "paint": null},
        "caches": {"gradient_tiles": {"hits": 3, "misses": {"evicted": 2}, "examples": []}},
        "memory": {"heap_bytes": 9, "changed_bytes": [["style/computed", 4]]},
        "error": "unexpected extra/reused frame: draws=1, renders=2",
        "frames": [{"kind": "notify", "site": "crates/engine-bench/src/run.rs:207"},
            {"kind": "draw", "site": "vendor/gpui/src/window.rs:2149"},
            {"kind": "render", "site": "crates/engine-bench/src/view.rs:99"},
            {"kind": "notify", "site": "crates/html/src/x.rs:5"},
            {"kind": "render", "site": "crates/engine-bench/src/view.rs:99"}],
        "frames_dropped": 0,
    }]});
    let s = render(&out);
    assert!(s.contains("#2 scroll/warm: built (built 1, replayed 0)"));
    assert!(s.contains(
        "frame_miss.scroll_deferreddraws at vendor/gpui/src/window/scroll_replay.rs:96: 1 deferred"
    ));
    assert!(s.contains("cause x2  frame_miss.key at a.rs:1\n"));
    assert!(s.contains("path  refused_no_layer=1"));
    assert!(s.contains("scroll animation frames 3 / builds 0"));
    assert!(s.contains("phase  layout=2.500"));
    assert!(s.contains("gradient_tiles: hits=3 misses: evicted=2"));
    assert!(s.contains("memory +4 style/computed"));
    assert!(s.contains("error  unexpected extra/reused frame"));
    assert!(s.contains("frame  draws=1 renders=2; requested after the first render: notify at crates/html/src/x.rs:5\n"));
    assert!(s.contains("frame    render at crates/engine-bench/src/view.rs:99\n"));
}

#[test]
fn a_captured_frame_reads_its_wait_submission_and_the_bench_copy_apart() {
    let out = json!({"schema": 1, "frames": [{
        "frame": 1, "mode": "edit", "step": "edit", "verdict": "replayed", "built": 0, "replayed": 1,
        "raster": {"plan": "within", "full_by": null, "shift_miss": "redraw_whole", "rects_count": 1,
            "raster_px": 1024, "rects": [[0, 0, 32, 32]], "gpu_wait_ms": 1.5, "submit_ms": 0.25,
            "capture_copy_ms": 1.125},
    }]});
    let s = render(&out);
    assert!(
        s.contains(
            "  raster within (not a move: redraw_whole) rects 1 raster_px 1024 [0,0 32x32]\n"
        ),
        "{s}"
    );
    assert!(
        s.contains("  gpu_wait 1.500 ms submit 0.250 ms capture_copy 1.125 ms\n"),
        "{s}"
    );
    let mut missing = out;
    missing["frames"][0]["raster"]["submit_ms"] = serde_json::Value::Null;
    missing["frames"][0]["raster"]["capture_copy_ms"] = serde_json::Value::Null;
    let s = render(&missing);
    assert!(s.contains("  gpu_wait 1.500 ms\n"), "{s}");
    assert!(!s.contains("submit"), "{s}");
    assert!(!s.contains("capture_copy"), "{s}");
}
