//! KaminIDE patch: the `[gpu]` line of a period: plans, batches, path intermediate.

use super::{BatchCost, FramePlan, PATHS};

#[test]
fn a_period_of_kept_frames_still_reports_its_plan() {
    let cost = BatchCost::default();
    cost.plan(FramePlan::Keep, 1000.0);
    let line = cost.take_line().unwrap();
    assert!(line.contains("plan keep=1 full=0 within=0"), "{line}");
    assert!(cost.take_line().is_none());
}

#[test]
fn painted_area_is_the_mean_of_drawn_frames_with_reasons() {
    let cost = BatchCost::default();
    cost.frame();
    cost.frame();
    cost.plan(FramePlan::Full("external"), 4_000_000.0);
    cost.plan(FramePlan::Within(10_000.0), 4_000_000.0);
    let line = cost.take_line().unwrap();
    assert!(
        line.contains("full=1 within=1 painted_kpx=2005 window_kpx=4000 full_by=external:1"),
        "{line}"
    );
}

#[test]
fn path_batches_report_their_count_and_the_intermediate_area_they_used() {
    let cost = BatchCost::default();
    cost.frame();
    // Two path batches of one frame: one used 3000 px, one reached nothing.
    cost.count(PATHS, 4, 2000.0);
    cost.intermediate(3000.0);
    cost.count(PATHS, 2, 400.0);
    cost.intermediate(0.0);
    cost.count(1, 7, 9000.0);
    let line = cost.take_line().unwrap();
    assert!(line.contains("paths: n=6 kpx=2 b=2 mid_kpx=3;"), "{line}");
    assert!(line.contains("quads: n=7 kpx=9 b=1;"), "{line}");
}

#[test]
fn resource_events_report_exact_bytes_even_without_a_drawn_frame() {
    let cost = BatchCost::default();
    cost.path_target_created(16 * 64 * 48);
    cost.path_target_created(16 * 32 * 24);
    cost.path_target_dropped();
    let line = cost.take_line().unwrap();
    assert!(
        line.contains("path_msaa_create=2 path_msaa_drop=1 path_msaa_alloc_bytes=61440"),
        "{line}"
    );
    assert!(cost.take_line().is_none());
}

#[test]
fn effect_events_report_exact_totals_and_reset_without_a_frame() {
    let cost = BatchCost::default();
    cost.mask_uploaded(24);
    cost.mask_uploaded(40);
    cost.effect_targets_dropped(2, 64);
    let line = cost.take_line().unwrap();
    assert!(line.contains("mask_upload_create=2 mask_upload_alloc_bytes=64 effect_target_drop=2 effect_target_release_bytes=64"), "{line}");
    assert!(cost.take_line().is_none());
}
