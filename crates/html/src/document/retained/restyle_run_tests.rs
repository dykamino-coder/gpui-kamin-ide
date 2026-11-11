//! Runs of restyles over the root-flow block records (`layout::reuse::rebase`):
//! which restyle records the blocks, which keeps the far ones, and when a run
//! that leaves no record stops wrapping blocks.

use super::paint_damage_tests::page;
use super::reuse_tests::{reused, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A document of three rows, all on the screen, whose class on `html`
/// recolours `body`: every record is near, so each such restyle drops all.
fn near(class: &str) -> String {
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;background:#fff;color:#000}} .r{{height:20px}} \
         html.paint body{{color:#123456}}</style></head>\
         <body><div class=r>a</div><div class=r>b</div><div class=r>c</div></body></html>"
    )
}

#[gpui::test]
fn restyles_that_drop_every_record_record_nothing_again(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &near(""));
    frame(cx);
    update(&view, cx, &near("x"));
    let tracked = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks())
    };
    // The first restyle drops the records of the settled frame; the store
    // starts over, and the frames that keep following rebuilds wrap nothing.
    update(&view, cx, &near("paint"));
    let after = tracked(&view, cx);
    for class in ["", "paint", "", "paint"] {
        update(&view, cx, &near(class));
    }
    assert_eq!(
        tracked(&view, cx),
        after,
        "a restyled block was recorded again"
    );
}

/// A restyle on every frame from the load on, with no quiet frame between
/// (a class animation): the first restyled frame records the blocks, and
/// every later one keeps the layout of the rows far from the screen.
#[gpui::test]
fn restyles_on_every_frame_keep_the_far_blocks_from_the_second_on(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    update(&view, cx, &page("paint", "row"));
    let (recorded, mut skipped, _) = reused(&view, cx);
    assert!(
        recorded > 400,
        "the first restyled frame recorded {recorded}"
    );
    for class in ["", "paint", "", "paint"] {
        update(&view, cx, &page(class, "row"));
        let (_, now, rebuilt) = reused(&view, cx);
        assert!(now - skipped > 400, "class '{class}': {}", now - skipped);
        assert_eq!(rebuilt, 0);
        skipped = now;
    }
}

/// Restyles with quiet frames between (an application's edits): a quiet
/// frame is replayed whole and records nothing, so the first restyled frame
/// records the blocks, and the next restyle keeps the far ones.
#[gpui::test]
fn restyles_between_quiet_frames_keep_the_far_blocks_from_the_second_on(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    frame(cx);
    update(&view, cx, &page("paint", "row"));
    let (recorded, skipped_before, _) = reused(&view, cx);
    assert!(
        recorded > 400,
        "the first restyled frame recorded {recorded}"
    );
    frame(cx);
    update(&view, cx, &page("", "row"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped - skipped_before > 400 && rebuilt == 0,
        "skipped {}, rebuilt {rebuilt}",
        skipped - skipped_before
    );
}

/// A block before the first float is the only one wrapped (`FlowCx::record`),
/// and a recolour of `body` leaves it no record: the wraps end with the
/// second restyle instead of repeating on each.
#[gpui::test]
fn restyles_that_leave_no_record_wrap_no_block_again(cx: &mut TestAppContext) {
    let floats = |class: &str| {
        near(class).replacen(
            "<div class=r>b</div>",
            "<div style='float:left;width:9px;height:9px'></div><p>b</p>",
            1,
        )
    };
    let (view, cx) = open(cx, &floats(""));
    frame(cx);
    update(&view, cx, &floats("paint"));
    update(&view, cx, &floats(""));
    let tracked = cx.update(|_, cx| view.read(cx).doc.tracked_blocks());
    for class in ["paint", "", "paint", ""] {
        update(&view, cx, &floats(class));
    }
    assert_eq!(
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks()),
        tracked,
        "a block was wrapped again"
    );
}
