//! The protocol form of the `why` journal: schema, stable codes, and what a
//! subscription records (nothing while off).

use super::*;

#[test]
fn json_carries_the_schema_codes_sites_and_frame_events() {
    let (_one, _sub) = test_reader();
    let line = line!() + 1;
    note("frame_unpublished", || "frame_miss.key".into());
    kamin_style::journal::cache_miss("gradient_tiles", "evicted", || "800x600 tile".into());
    trace::mark("render");
    let value = to_json(&take());
    assert_eq!(value["schema"], SCHEMA);
    let here = format!("crates/html/src/why/tests.rs:{line}");
    let cause = value["causes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "frame_unpublished")
        .expect("the noted cause");
    assert_eq!(cause["site"], here);
    assert_eq!(cause["detail"], "frame_miss.key");
    let tiles = &value["caches"]["gradient_tiles"];
    assert_eq!(tiles["misses"]["evicted"], 1);
    assert_eq!(tiles["examples"][0]["code"], "evicted");
    let render = value["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "render")
        .expect("the render mark");
    assert!(
        render["site"]
            .as_str()
            .unwrap()
            .starts_with("crates/html/src/why/tests.rs:")
    );
    for key in ["causes_dropped", "frames_dropped"] {
        assert!(value[key].is_u64(), "{key}");
    }
}

#[test]
fn off_records_nothing() {
    let one = test_reader();
    drop(one.1);
    assert!(!enabled());
    note("off", || unreachable!("detail of a disabled journal"));
    trace::mark("off");
    let entry = take();
    assert!(entry.journal.causes.iter().all(|c| c.code != "off"));
    assert!(entry.frames.iter().all(|e| e.kind != "off"));
}

#[test]
fn codes_are_unique_and_cover_frame_misses_and_refusals() {
    let unique: std::collections::HashSet<_> = CODES.iter().collect();
    assert_eq!(unique.len(), CODES.len());
    for code in [
        "frame_miss.key",
        "frame_miss.scroll_deferreddraws",
        "paint_path.refused_no_layer",
    ] {
        assert!(is_code(code), "{code}");
    }
    for code in ["frame_miss.nope", "paint_path.replayed", "nope"] {
        assert!(!is_code(code), "{code}");
    }
}
