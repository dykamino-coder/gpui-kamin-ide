//! A composition group painted inside a deferred draw replays with its frame.

use super::box_repaint_tests::frames;
use super::tests::{frame, open};
use gpui::TestAppContext;

/// A positioned box with a `z-index` paints in a deferred draw, its blended
/// child in a composition group of that draw: the frame replays with the
/// group copied (`gpui::Window::reuse_deferred_paint`) and draws the groups
/// and labels of the built frame.
#[gpui::test]
fn a_group_painted_by_a_deferred_draw_replays_with_the_frame(cx: &mut TestAppContext) {
    let page = "<div style='position:relative;z-index:1;width:60px;height:60px;background:lime'>\
        <div style='mix-blend-mode:multiply;width:30px;height:30px;background:red'></div></div>";
    let (view, cx) = open(cx, page);
    frame(cx);
    let built = cx.update(|window, _| window.painted_groups());
    assert!(!built.0.is_empty(), "the blended child made no group");
    let (builds, replays) = frames(&view, cx);
    for _ in 0..3 {
        frame(cx);
        assert_eq!(cx.update(|window, _| window.painted_groups()), built);
    }
    assert_eq!(frames(&view, cx), (builds, replays + 3));
}
