//! Root phase attribution, checkpoint lifetime and disabled recording.

use super::*;

fn opts() -> crate::RenderOpts {
    crate::RenderOpts {
        viewport: (1200., 824.),
        text: Default::default(),
        normal_line_height: 1.31,
        cx: Default::default(),
        frag: Default::default(),
    }
}

#[test]
fn work_calls_count_actual_entry_points_and_reset_at_checkpoint() {
    let (_one, reader) = crate::why::test_reader();
    let journal = Journal::default();
    assert_eq!(journal.take()["work_calls"], work_json([0; 5]));
    for work in [
        Work::UpdateRendering,
        Work::RenderViewport,
        Work::RequestLayout,
        Work::Prepaint,
        Work::Paint,
        Work::Paint,
    ] {
        journal.called(work);
    }
    assert_eq!(journal.take()["work_calls"], work_json([1, 1, 1, 1, 2]));
    assert_eq!(journal.take()["work_calls"], work_json([0; 5]));
    drop(reader);
    let disabled = Journal::default();
    disabled.called(Work::Paint);
    assert!(disabled.0.borrow().is_none());
}

#[test]
fn nested_rebuilds_partition_the_inclusive_prepaint_interval() {
    let (_one, _reader) = crate::why::test_reader();
    let journal = Journal::default();
    let attempt = journal.begin(1, 2, false, &opts(), true);
    let now = Instant::now();
    let at = |ms| now + Duration::from_millis(ms);
    let mut parent = attempt.enter_at(Phase::Prepaint, at(0));
    for (phase, start, end) in [
        (Phase::Build, 2, 5),
        (Phase::RequestLayout, 6, 10),
        (Phase::Layout, 11, 16),
        (Phase::Build, 17, 20),
    ] {
        attempt.enter_at(phase, at(start)).finish_at(at(end));
    }
    parent.finish_at(at(30));
    let out = journal.take();
    assert_eq!(out["clock"], "wall");
    let root = &out["attempts"][0];
    assert_eq!(root["phases_ms"]["prepaint"]["inclusive"], 30.);
    assert_eq!(root["phases_ms"]["prepaint"]["exclusive"], 15.);
    assert_eq!(root["phases_ms"]["build"]["inclusive"], 6.);
    assert_eq!(root["phases_ms"]["build"]["calls"], 2);
    assert_eq!(root["exclusive_total_ms"], 30.);
    assert_eq!(root["painted"], false);
}

#[test]
fn checkpoints_cancel_abandoned_gaps_and_scopes_without_crossing_documents() {
    let (_one, _reader) = crate::why::test_reader();
    let journal = Journal::default();
    let other = Journal::default();
    let old = journal.begin(1, 10, true, &opts(), true);
    old.requested(1.25);
    let mut unfinished = old.enter_at(Phase::Paint, Instant::now());
    let out = journal.take();
    assert_eq!(out["attempts"][0]["pending_host_layout"], true);
    unfinished.finish_at(Instant::now());
    drop(old.prepaint());
    let next = journal.begin(1, 11, false, &opts(), true);
    let unrelated = other.begin(2, 99, false, &opts(), false);
    next.requested(1.);
    drop(unrelated.prepaint());
    let unrelated_out = other.take();
    assert_eq!(unrelated_out["attempts"][0]["document"], 2);
    assert_eq!(
        unrelated_out["attempts"][0]["phases_ms"][NAMES[3]]["calls"],
        0
    );
    drop(next.prepaint());
    let out = journal.take();
    assert_eq!(out["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(out["attempts"][0]["tree"], 11);
    assert_eq!(out["attempts"][0]["attempt"], 2);
    assert_eq!(out["attempts"][0]["pending_host_layout"], false);
    assert_eq!(out["attempts"][0]["phases_ms"][NAMES[3]]["calls"], 1);
    assert!(journal.take()["attempts"].as_array().unwrap().is_empty());
}

#[test]
fn revisions_share_the_bounded_owner_and_report_dropped_attempts() {
    let (_one, _reader) = crate::why::test_reader();
    let journal = Journal::default();
    let first = journal.begin(1, 1, true, &opts(), true);
    first.painted();
    let revision = journal.clone();
    for tree in 2..=LIMIT as u64 + 1 {
        revision.begin(1, tree, false, &opts(), true);
    }
    let out = revision.take();
    assert_eq!(out["attempts"].as_array().unwrap().len(), LIMIT);
    assert_eq!(out["dropped"], 1);
    assert_eq!(out["attempts"][0]["painted"], true);
    assert!(journal.take()["attempts"].as_array().unwrap().is_empty());
}

#[test]
fn disabled_recording_has_no_attempt_storage_or_clock_scope() {
    let (_one, reader) = crate::why::test_reader();
    let observed = Journal::default();
    let old = observed.begin(2, 2, false, &opts(), false);
    let scope = old.enter(Phase::Build);
    drop(reader);
    assert!(!crate::why::enabled());
    let journal = Journal::default();
    let attempt = journal.begin(1, 1, false, &opts(), false);
    assert!(journal.0.borrow().is_none());
    assert!(attempt.0.is_none());
    assert!(attempt.enter(Phase::Build).0.is_none());
    attempt.requested(1.);
    assert!(attempt.prepaint().0.is_none());
    drop(scope);
    assert!(!old.0.as_ref().unwrap().borrow().live);
    assert!(old.enter(Phase::Paint).0.is_none());
}
