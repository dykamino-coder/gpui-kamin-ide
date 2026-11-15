//! Replay of root-flow blocks whose floats stay inside a formatting context
//! root of the block (`layout::reuse::plain::escaping_floats`): a multicol
//! sheet of floated items replays on screen across an edit elsewhere and
//! across a width change it does not depend on, and paints what a fresh
//! document paints. A float that leaves its block keeps the block built.
//!
//! Each target starts with an empty block: the test window's mouse rests at
//! its top left corner, and a hovered text block registers its cursor and
//! selection tracking, which no replay carries (`InkScope`).

use super::layered_replay_tests::{edit, page, replayed, scene};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

const PAD: &str = "<div style='height:20px'></div>";

/// A fixed-width multicol sheet of items with a float each (the shape of the
/// `columns-1600` bench page).
fn sheet(items: usize) -> String {
    let items: String = (0..items)
        .map(|i| {
            format!(
                "<p style='margin:0 0 3px;background:#def'>\
                 <span style='float:left;width:30px;height:40px;background:#a73'></span>\
                 item {i} one two three four five six seven</p>"
            )
        })
        .collect();
    format!(
        "{PAD}<div style='width:780px;columns:4;column-gap:12px'>\
         <h2 style='column-span:all;margin:2px'>Section</h2>{items}</div>"
    )
}

#[gpui::test]
fn a_multicol_sheet_with_floats_replays_and_paints_as_a_fresh_document(cx: &mut TestAppContext) {
    let target = sheet(40);
    let (count, _, after, fresh) = edit(cx, &target, &page("", &target, "one"));
    assert_eq!(count, 3, "the pad, the sheet and the witness replay");
    assert_eq!(after, fresh, "the replayed frame differs from a fresh one");
}

#[gpui::test]
fn a_block_whose_float_leaves_it_is_built(cx: &mut TestAppContext) {
    let target = format!(
        "{PAD}<div><p><span style='float:left;width:30px;height:90px;background:#a73'>\
         </span>text</p></div>"
    );
    let (count, _, after, fresh) = edit(cx, &target, &page("", &target, "one"));
    // The float reaches the blocks after it: neither it nor they replay.
    assert_eq!(count, 1, "only the pad before the float replays");
    assert_eq!(after, fresh);
}

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

#[gpui::test]
fn a_fixed_width_sheet_with_floats_replays_across_a_width_change(cx: &mut TestAppContext) {
    let html = format!("<body style='margin:0'>{}</body>", sheet(40));
    let (view, vcx) = open(cx, &html);
    frame(vcx);
    for width in [1201., 1200., 1201.] {
        resize(&view, vcx, width);
    }
    let start = replayed(&view, vcx);
    resize(&view, vcx, 1200.);
    let count = replayed(&view, vcx) - start;
    assert!(count >= 1, "the sheet was built again: {count}");
    let after = scene(vcx);
    let (fresh_view, fresh) = open(cx, &html);
    resize(&fresh_view, fresh, 1200.);
    assert_eq!(
        after,
        scene(fresh),
        "the replayed frame differs from a fresh one"
    );
}
