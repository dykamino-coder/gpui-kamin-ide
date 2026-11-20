//! Paint-only edits of boxes off the screen (`revision::layer_write`,
//! `Store::unpainted`): a box under a block the frame skipped keeps the frame
//! without a build; a box the frame painted off the screen keeps it too, its
//! record deferred (`layout::reuse::deferred`). The frames that bring either
//! onto the screen (a taller window, a taller viewport) equal a fresh
//! document's; a box the frame showed, even in part, is recorded again or
//! builds the frame, never left stale.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext, px, size};

/// Edits off the screen that the layer cannot record (`revision::offscreen`).
#[path = "offscreen_tests.rs"]
mod offscreen;

/// Rows of 20px; `edit` is spliced into the start tag of row `at`. Row 14
/// straddles the bottom of the 300px window (`small`); the empty block after row 13
/// paints nothing and reaches into the viewport (its edit: `at == HOLE`).
const HOLE: usize = 999;

fn page(at: usize, edit: &str) -> String {
    framed(at, edit, false)
}

/// `page`, its rows in one block that reaches onto the screen (`wrapped`):
/// the frame paints every row of it, off the screen too, into the layer.
fn framed(at: usize, edit: &str, wrapped: bool) -> String {
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
         </style></head><body>{open}{rows}{close}</body></html>",
        open = if wrapped { "<div>" } else { "" },
        close = if wrapped { "</div>" } else { "" },
    )
}

fn tall(view: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4200.));
    frame(cx);
}

/// The window, and so the content mask the frame shows, at `height`.
fn window(cx: &mut VisualTestContext, height: f32) {
    cx.simulate_resize(size(px(400.), px(height)));
    frame(cx);
}

fn small(cx: &mut VisualTestContext) {
    window(cx, 300.);
}

fn grown(cx: &mut VisualTestContext) {
    window(cx, 4200.);
}

/// A fresh document of `html` in the small window, then `then` (a taller
/// window or viewport).
fn fresh(
    cx: &mut TestAppContext,
    html: &str,
    then: impl FnOnce(&gpui::Entity<View>, &mut VisualTestContext),
) -> (Vec<String>, Vec<String>) {
    let (view, fresh) = open(cx, html);
    small(fresh);
    let first = quads(fresh);
    then(&view, fresh);
    (first, quads(fresh))
}

/// The edit of row `at` from a settled frame: whether it kept the frame
/// without a build, how many box records wait, the scene after it and after
/// `then`.
fn edited(
    cx: &mut TestAppContext,
    at: usize,
    wrapped: bool,
    then: impl FnOnce(&gpui::Entity<View>, &mut VisualTestContext),
) -> (bool, usize, Vec<String>, Vec<String>) {
    let page = |edit: &str| framed(at, edit, wrapped);
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    small(vcx);
    update(&view, vcx, &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit"));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let waiting = vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints());
    let after = quads(vcx);
    then(&view, vcx);
    (kept, waiting, after, quads(vcx))
}

/// The edit of row `at` keeps the frame (`keeps`), defers the record of its
/// box off the screen (`defers`), and both its frame and the frame after a
/// taller window (`grows`) or a taller viewport equal a fresh document's.
fn check(
    cx: &mut TestAppContext,
    at: usize,
    wrapped: bool,
    keeps: bool,
    defers: bool,
    grows: bool,
) {
    let then = move |view: &gpui::Entity<View>, cx: &mut VisualTestContext| match grows {
        true => grown(cx),
        false => tall(view, cx),
    };
    let (kept, waiting, edited_now, edited_then) = edited(cx, at, wrapped, then);
    let (fresh_now, fresh_then) = fresh(cx, &framed(at, " data-edit", wrapped), then);
    assert!(
        !fresh_now.is_empty() && !fresh_then.is_empty(),
        "empty reference scene"
    );
    assert_eq!(kept, keeps, "row {at}: the frame was kept: {kept}");
    assert_eq!(waiting > 0, defers, "row {at}: records waiting: {waiting}");
    assert_eq!(
        edited_now, fresh_now,
        "row {at}: the edited frame differs from a fresh one"
    );
    assert_eq!(
        edited_then, fresh_then,
        "row {at}: on the screen it differs from a fresh one"
    );
}

#[gpui::test]
fn a_box_far_below_the_screen_keeps_the_frame_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, 140, false, true, true, true);
}

#[gpui::test]
fn a_box_far_below_the_screen_shows_as_fresh_in_a_taller_viewport(cx: &mut TestAppContext) {
    check(cx, 140, false, true, true, false);
}

#[gpui::test]
fn a_box_in_part_on_the_screen_is_recorded_again_as_fresh(cx: &mut TestAppContext) {
    check(cx, 14, false, true, false, true);
}

#[gpui::test]
fn a_painted_box_below_the_screen_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, 40, true, true, true, true);
}

#[gpui::test]
fn a_painted_box_below_the_screen_shows_as_fresh_in_a_taller_viewport(cx: &mut TestAppContext) {
    check(cx, 40, true, true, true, false);
}

#[gpui::test]
fn a_painted_box_on_the_screen_is_recorded_now(cx: &mut TestAppContext) {
    check(cx, 3, true, true, false, true);
}

#[gpui::test]
fn a_taller_window_shows_more_rows(cx: &mut TestAppContext) {
    let (before, after) = fresh(cx, &page(140, ""), |_, cx| grown(cx));
    assert_ne!(before, after, "the taller window shows the rows below");
}

#[gpui::test]
fn an_empty_block_reaching_into_the_screen_is_not_left_stale(cx: &mut TestAppContext) {
    let then = |_: &gpui::Entity<View>, cx: &mut VisualTestContext| frame(cx);
    let (_, _, edited_now, _) = edited(cx, HOLE, false, then);
    let (fresh_now, _) = fresh(cx, &page(HOLE, " data-edit"), then);
    assert_eq!(
        edited_now, fresh_now,
        "the empty block's new fill differs from a fresh one"
    );
}

/// Grow both the window clip and the document viewport so the edited row
/// contributes paint. An unedited fresh document is the negative control.
#[gpui::test]
fn the_revealed_row_contributes_its_changed_fill(cx: &mut TestAppContext) {
    let reveal = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4200.));
        window(cx, 4200.);
    };

    let row = 140;
    let (kept, waiting, _, after) = edited(cx, row, false, reveal);
    let (_, fresh_edit) = fresh(cx, &page(row, " data-edit"), reveal);
    let (_, unchanged) = fresh(cx, &page(row, ""), reveal);
    assert!(kept && waiting > 0);
    assert!(!fresh_edit.is_empty());
    assert_ne!(fresh_edit, unchanged, "the changed row must be visible");
    assert_eq!(after, fresh_edit);
}
