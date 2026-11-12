//! Фильтр и конец окна пробника, отсутствие следа в счётчиках, граница строки.

use super::*;

const NAMES: [&str; 3] = ["layout", "paint", "raster_gpu_readback"];

#[test]
fn filter_selects_the_phase_and_frame_range() {
    let filter = Filter::parse("raster_gpu_readback@3-5", &NAMES).unwrap();
    assert!(filter.matches(2, 3) && filter.matches(2, 5));
    assert!(!filter.matches(2, 2) && !filter.matches(2, 6));
    assert!(!filter.matches(1, 4) && !filter.matches(OUTSIDE, 4));
}

#[test]
fn filter_without_frames_covers_every_frame_of_the_phase() {
    let filter = Filter::parse("paint", &NAMES).unwrap();
    assert!(filter.matches(1, 0) && filter.matches(1, u64::MAX));
    assert!(!filter.matches(0, 1));
    let one = Filter::parse("paint@7", &NAMES).unwrap();
    assert!(one.matches(1, 7) && !one.matches(1, 6) && !one.matches(1, 8));
}

#[test]
fn filter_names_process_start_and_any_phase() {
    let start = Filter::parse("outside@0", &NAMES).unwrap();
    assert!(start.matches(OUTSIDE, 0));
    assert!(!start.matches(OUTSIDE, 1) && !start.matches(0, 0));
    let any = Filter::parse("*@2", &NAMES).unwrap();
    assert!(any.matches(0, 2) && any.matches(OUTSIDE, 2) && !any.matches(0, 1));
}

#[test]
fn filter_rejects_unknown_phases_and_malformed_ranges() {
    for spec in [
        "",
        "rasterize",
        "paint@",
        "paint@x",
        "paint@5-3",
        "paint@1-",
        "paint@-2",
    ] {
        assert_eq!(Filter::parse(spec, &NAMES), None, "{spec}");
    }
}

#[test]
fn window_ends_when_the_last_frame_leaves_the_until_phase() {
    let (build, layout, other) = (4, 5, 10);
    let mut window = Until::Before;
    for (phase, frame) in [(other, 1), (build, 1), (layout, 1), (other, 2)] {
        window = window.step(build, 2, phase, frame);
        assert_eq!(
            window,
            Until::Before,
            "earlier frames leave the window open"
        );
    }
    window = window.step(build, 2, build, 2);
    assert_eq!(window, Until::Inside);
    window = window.step(build, 2, layout, 2);
    assert_eq!(window, Until::Closed);
    assert_eq!(
        window.step(build, 2, build, 2),
        Until::Closed,
        "closed for good"
    );
    assert_eq!(Until::Open.step(build, 2, layout, 2), Until::Open);
}

#[test]
fn disarmed_probe_leaves_the_allocation_counters_alone() {
    let before = super::super::Snapshot::read();
    record(b'a', 64, std::ptr::null());
    release(64, std::ptr::null());
    let after = super::super::Snapshot::read();
    assert_eq!((before.calls, before.bytes), (after.calls, after.bytes));
}

#[test]
fn line_refuses_text_past_its_stack_buffer() {
    let mut line = Line {
        bytes: [0; 1024],
        length: 1020,
    };
    assert!(line.write_str("abcd").is_ok());
    assert!(line.write_str("e").is_err());
    assert_eq!(line.length, 1024);
}

#[test]
fn lines_carry_the_pointer_after_the_size() {
    let (line, text) = head(b'f', 16, 0xbeef0 as *const u8);
    assert!(text.is_ok());
    let line = std::str::from_utf8(&line.bytes[..line.length]).unwrap();
    // Кадр и фаза общие для процесса: другие тесты стенда могут их двигать.
    let fields: Vec<_> = line.split(' ').collect();
    assert_eq!(
        (fields[0], fields[3], fields[4]),
        ("f", "16", "beef0"),
        "{line}"
    );
    assert_eq!(fields.len(), 5, "a free has no stack: {line}");
}
