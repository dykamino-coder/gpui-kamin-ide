//! The flow key over a rebuild of the same source DOM (`layout::reuse::rebase`):
//! a rebuild that keeps records strips the previous `html`/`body` boxes from
//! the key it carries, one that keeps none starts the store over.

use super::paint_damage_tests::page;
use super::reuse_tests::{reused, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// `page` below a tall first block with `text`: every row is far from the
/// screen, and an edit of the first block (always built) records nothing.
fn far_rows(class: &str, text: &str) -> String {
    page(class, "row").replacen(
        "<body>",
        &format!("<body><div style='height:3000px'>{text}</div>"),
        1,
    )
}

#[gpui::test]
fn a_rebuild_keeps_the_flow_key_without_the_previous_root_boxes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &far_rows("", "top"));
    frame(cx);
    // The second build records the far rows; the third skips them all and
    // records nothing, so its flow key is the store's alone.
    update(&view, cx, &far_rows("", "edited"));
    update(&view, cx, &far_rows("", "top"));
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    assert_eq!(store.key_holds_boxes(), Some(true));
    let html = far_rows("paint", "top");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&html, "").unwrap()));
    assert_eq!(
        store.key_holds_boxes(),
        Some(false),
        "the flow key held the previous html/body boxes over the rebuild"
    );
    let (_, skipped_before, _) = reused(&view, cx);
    frame(cx);
    // The stripped key still counts as the same flow: the kept rows skip.
    let (_, skipped, _) = reused(&view, cx);
    assert!(
        skipped - skipped_before > 400,
        "{}",
        skipped - skipped_before
    );
    assert_eq!(store.key_holds_boxes(), Some(true));
}

#[gpui::test]
fn a_first_restyle_carries_the_flow_key_and_records_in_its_own_frame(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    assert_eq!(store.key_holds_boxes(), Some(true));
    let html = page("paint", "row");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&html, "").unwrap()));
    // No record was made or tried yet: the key is carried, without the boxes.
    assert_eq!(
        store.key_holds_boxes(),
        Some(false),
        "the flow key held the previous html/body boxes over the rebuild"
    );
    let tracked = cx.update(|_, cx| view.read(cx).doc.tracked_blocks());
    frame(cx);
    // The rebuild's own frame records the blocks for the next restyle.
    let wrapped = cx.update(|_, cx| view.read(cx).doc.tracked_blocks()) - tracked;
    assert!(wrapped > 400, "the first restyled frame wrapped {wrapped}");
}

#[gpui::test]
fn a_rebuild_that_drops_every_record_drops_the_previous_root_boxes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    // The first restyled frame records the rows; a new margin leaves none.
    update(&view, cx, &page("paint", "row"));
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let html = page("layout", "row");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&html, "").unwrap()));
    assert_eq!(
        store.key_holds_boxes(),
        None,
        "a flow key outlived the rebuild"
    );
    let tracked = cx.update(|_, cx| view.read(cx).doc.tracked_blocks());
    frame(cx);
    // As a fresh document's, the rebuild's own frame records nothing.
    assert_eq!(
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks()),
        tracked
    );
}
