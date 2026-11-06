//! Записи повтора поддерева рядом с записями пропуска: блок, которого
//! заместитель заменит при любой ширине записи, второй записи не получает.

use super::super::tests::{View, frame, open};
use super::{fixed_rows, resize, reused, text_rows};
use gpui::{TestAppContext, VisualTestContext};

fn replay_tracked(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replay_tracked_blocks())
}

/// A build at a changed width records skips only for blocks whose layout does
/// not follow the width: their placeholders serve every later width, so they
/// get no second, subtree replay record (a copy of their frame output).
#[gpui::test]
fn a_resized_build_does_not_record_skipped_blocks_twice(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &fixed_rows(""));
    frame(cx);
    let (recorded, _, _) = reused(&view, cx);
    let wraps = replay_tracked(&view, cx);
    resize(&view, cx, 401.);
    assert!(
        reused(&view, cx).0 - recorded > 500,
        "fixed-width blocks were not recorded at the new width"
    );
    assert_eq!(
        replay_tracked(&view, cx) - wraps,
        0,
        "skip-recorded blocks were wrapped for subtree replay too"
    );
}

/// Blocks wrapped for subtree replay by each of two warm settled builds of
/// `page` that rebuild row 300 (the edge rows of the flow are built and
/// wrapped by every build).
fn warm_wraps(cx: &mut TestAppContext, page: &str) -> (u64, u64) {
    let (view, cx) = open(cx, page);
    frame(cx);
    let mut wraps = Vec::new();
    for edit in ["x", "y", "z"] {
        let edited = page.replace("row 300", &format!("row {edit}300"));
        cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&edited, "").unwrap()));
        frame(cx);
        wraps.push(replay_tracked(&view, cx));
    }
    // The first edit is the first settled build; the next two are warm.
    (wraps[1] - wraps[0], wraps[2] - wraps[1])
}

/// A warm settled build wraps a rebuilt skip-recorded block for subtree
/// replay only while that record may serve another width: not a block that
/// fills the width, and not one whose last replay record read the width
/// from outside. Its skip record serves the same width.
#[gpui::test]
fn a_warm_settled_build_records_for_replay_only_what_may_serve_another_width(
    cx: &mut TestAppContext,
) {
    let fixed = warm_wraps(cx, &fixed_rows(""));
    let filling = warm_wraps(cx, &text_rows(""));
    assert_eq!(
        fixed.0,
        filling.0 + 1,
        "a fixed-width block was not wrapped, or a filling one was"
    );
    assert_eq!(
        fixed.1, filling.1,
        "a block whose replay record read the width was wrapped again"
    );
}
