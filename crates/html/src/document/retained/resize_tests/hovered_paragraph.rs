//! A fixed-width paragraph under the pointer across window width changes.
//! Its selection listeners test the hit when an event comes, not where the
//! pointer was at paint (`interactive::selection::track_selection`), so the
//! paragraph the pointer rests on is recorded and replayed like any other
//! block.

use super::super::layered_replay_tests::scene;
use super::super::tests::{View, open};
use super::{replayed, resize};
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};

/// One long paragraph of fixed width, taller than the window, in mixed spans.
/// Its background is ink on screen: the test text system rasterizes no
/// glyphs, and the lines culled below the window offer ink far from it
/// (`flat::cull`), which alone would make the block far.
fn page() -> String {
    let spans: String = (0..300)
        .map(|i| format!("<span>word{i} AV ffi fi antidisestablishmentarianism. </span>"))
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0;font:16px Arial}} \
         p{{width:770px;margin:0;line-height:1.3;text-align:justify;background:#ffe}} \
         span:nth-child(3n){{font-weight:bold;letter-spacing:.25px}}</style></head>\
         <body><p>{spans}</p></body></html>"
    )
}

/// The pointer at `at` on the paragraph; the window resized to 800, 801, 800
/// and 801: blocks replayed by the last two frames.
fn hovered(cx: &mut VisualTestContext, view: &gpui::Entity<View>) -> u64 {
    cx.simulate_mouse_move(point(px(200.), px(100.)), None, Modifiers::default());
    resize(view, cx, 800.);
    resize(view, cx, 801.);
    let before = replayed(view, cx);
    resize(view, cx, 800.);
    resize(view, cx, 801.);
    replayed(view, cx) - before
}

#[gpui::test]
fn the_paragraph_under_the_pointer_is_replayed_across_width_changes(cx: &mut TestAppContext) {
    let html = page();
    let (view, vcx) = open(cx, &html);
    assert_eq!(hovered(vcx, &view), 2, "the hovered paragraph was built at a width change");
    let kept = scene(vcx);
    let (fresh, fcx) = open(cx, &html);
    fcx.simulate_mouse_move(point(px(200.), px(100.)), None, Modifiers::default());
    resize(&fresh, fcx, 801.);
    assert_eq!(kept, scene(fcx), "the replayed frame at 801 differs from a fresh one");
}
