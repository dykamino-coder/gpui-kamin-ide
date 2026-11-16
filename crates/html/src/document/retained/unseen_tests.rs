//! Paint-only edits of boxes off the screen (`revision::layer_write`,
//! `Store::unpainted`): a box under a block the frame skipped keeps the frame
//! without a build, and the frames that bring it onto the screen (a scroll, a
//! taller viewport) equal a fresh document's; a box the frame painted, even in
//! part, is recorded again or builds the frame, never left stale.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{
    Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px,
};

/// Rows of 20px; `edit` is spliced into the start tag of row `at`. Row 14
/// straddles the bottom of the 300px viewport; the empty block after row 13
/// paints nothing and reaches into the viewport (its edit: `at == HOLE`).
const HOLE: usize = 999;

fn page(at: usize, edit: &str) -> String {
    let rows: String = (0..200)
        .map(|i| {
            let mark = if i == at { edit } else { "" };
            let hole = match (i, at) {
                (13, HOLE) => format!("<div class=e{edit}></div>"),
                (13, _) => "<div class=e></div>".to_owned(),
                _ => String::new(),
            };
            format!("<div class=r{mark}>row {i}</div>{hole}")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         .r{{height:20px}} .e{{height:5px;margin-bottom:-5px}} [data-edit]{{background:#eee}}\
         </style></head><body>{rows}</body></html>"
    )
}

fn frames(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.retained_frames())
}

/// The painted quads in paint order, without their draw orders.
fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|mut q| {
            q.order = 0;
            format!("{q:?}")
        });
        quads.collect()
    })
}

fn tall(view: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4200.));
    frame(cx);
}

fn scroll(cx: &mut VisualTestContext, by: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-by))),
        modifiers: Modifiers::default(),
        ..Default::default()
    });
    frame(cx);
}

/// A fresh document of `html`, then `then` (a scroll or a taller viewport).
fn fresh(
    cx: &mut TestAppContext,
    html: &str,
    then: impl FnOnce(&gpui::Entity<View>, &mut VisualTestContext),
) -> (Vec<String>, Vec<String>) {
    let (view, fresh) = open(cx, html);
    frame(fresh);
    let first = quads(fresh);
    then(&view, fresh);
    (first, quads(fresh))
}

/// The edit of row `at` from a settled frame: whether it kept the frame
/// without a build, the scene after it and after `then`.
fn edited(
    cx: &mut TestAppContext,
    at: usize,
    then: impl FnOnce(&gpui::Entity<View>, &mut VisualTestContext),
) -> (bool, Vec<String>, Vec<String>) {
    let (view, vcx) = settled(cx, &page(at, ""), &page(at, " data-x"));
    update(&view, vcx, &page(at, ""));
    update(&view, vcx, &page(at, ""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(at, " data-edit"));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let after = quads(vcx);
    then(&view, vcx);
    (kept, after, quads(vcx))
}

fn check(cx: &mut TestAppContext, at: usize, keeps: bool, scrolled: bool) {
    let then = move |view: &gpui::Entity<View>, cx: &mut VisualTestContext| match scrolled {
        true => scroll(cx, 2600.),
        false => tall(view, cx),
    };
    let (kept, edited_now, edited_then) = edited(cx, at, then);
    let (fresh_now, fresh_then) = fresh(cx, &page(at, " data-edit"), then);
    assert_eq!(kept, keeps, "row {at}: the frame was kept: {kept}");
    assert_eq!(edited_now, fresh_now, "row {at}: the edited frame differs from a fresh one");
    assert_eq!(edited_then, fresh_then, "row {at}: on the screen it differs from a fresh one");
}

#[gpui::test]
fn a_box_far_below_the_screen_keeps_the_frame_and_scrolls_in_as_fresh(cx: &mut TestAppContext) {
    check(cx, 140, true, true);
}

#[gpui::test]
fn a_box_far_below_the_screen_shows_as_fresh_in_a_taller_viewport(cx: &mut TestAppContext) {
    check(cx, 140, true, false);
}

#[gpui::test]
fn a_box_in_part_on_the_screen_is_recorded_again_as_fresh(cx: &mut TestAppContext) {
    check(cx, 14, true, true);
}

#[gpui::test]
fn a_scroll_moves_the_rows(cx: &mut TestAppContext) {
    let (before, after) = fresh(cx, &page(140, ""), |_, cx| scroll(cx, 2600.));
    assert_ne!(before, after, "the wheel scrolled the document");
}

#[gpui::test]
fn an_empty_block_reaching_into_the_screen_is_not_left_stale(cx: &mut TestAppContext) {
    let then = |_: &gpui::Entity<View>, cx: &mut VisualTestContext| scroll(cx, 0.);
    let (_, edited_now, _) = edited(cx, HOLE, then);
    let (fresh_now, _) = fresh(cx, &page(HOLE, " data-edit"), then);
    assert_eq!(edited_now, fresh_now, "the empty block's new fill differs from a fresh one");
}
