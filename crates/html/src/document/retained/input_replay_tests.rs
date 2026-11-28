//! An input that changes no element state keeps the recorded frame
//! (`gpui::Window::interaction_epoch` counts only inputs whose listeners redraw):
//! pointer moves over a settled document replay it, and the pointer that enters
//! a box with a hover style repaints that box into the replayed frame
//! (`Builder::repaint_boxes`). Each frame equals a fresh document's under the
//! same pointer.

use super::box_repaint_tests::{frames, quads};
use super::tests::{frame, open};
use gpui::{Modifiers, Pixels, Point, TestAppContext, point, px};

const PAGE: &str = "<!doctype html><html><head><style>html,body{margin:0;background:#fff;color:#000}\
     #s{background:red;height:20px} #s:hover{background:green}\
     p{width:120px;font:16px/20px sans-serif;margin:0}</style></head>\
     <body><div id=s></div><p>one two three four five six</p></body></html>";

/// A point over the paragraph, away from the hover box.
fn text(x: f32) -> Point<Pixels> {
    point(px(x), px(40.))
}

fn fresh(cx: &mut TestAppContext, pointer: Point<Pixels>) -> Vec<String> {
    let (_, fresh) = open(cx, PAGE);
    fresh.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(fresh);
    frame(fresh);
    quads(fresh)
}

#[gpui::test]
fn pointer_moves_that_change_nothing_replay_the_frame(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, PAGE);
    vcx.simulate_mouse_move(text(10.), None, Modifiers::default());
    frame(vcx);
    frame(vcx);
    let (builds, replays) = frames(&view, vcx);
    for x in [20., 30., 40., 50.] {
        vcx.simulate_mouse_move(text(x), None, Modifiers::default());
        frame(vcx);
    }
    let (after_builds, after_replays) = frames(&view, vcx);
    assert_eq!(
        after_builds, builds,
        "a pointer move built the document again"
    );
    assert!(
        after_replays >= replays + 4,
        "every frame after a move replays"
    );
    assert_eq!(quads(vcx), fresh(cx, text(50.)));
}

#[gpui::test]
fn the_pointer_entering_a_hover_box_shows_its_hover(cx: &mut TestAppContext) {
    let on_box = point(px(10.), px(10.));
    let (entered_fresh, left_fresh) = (fresh(cx, on_box), fresh(cx, text(10.)));
    assert_ne!(entered_fresh, left_fresh, "the hover style paints");
    let (view, vcx) = open(cx, PAGE);
    vcx.simulate_mouse_move(text(10.), None, Modifiers::default());
    frame(vcx);
    frame(vcx);
    let (builds, _) = frames(&view, vcx);
    vcx.simulate_mouse_move(on_box, None, Modifiers::default());
    frame(vcx);
    frame(vcx);
    assert_eq!(quads(vcx), entered_fresh, "the pointer entered");
    vcx.simulate_mouse_move(text(10.), None, Modifiers::default());
    frame(vcx);
    frame(vcx);
    assert_eq!(quads(vcx), left_fresh, "the pointer left");
    // A move that enters no hover box changes nothing.
    let (entered, _) = frames(&view, vcx);
    vcx.simulate_mouse_move(text(30.), None, Modifiers::default());
    frame(vcx);
    assert_eq!(
        frames(&view, vcx).0,
        entered,
        "a move over the text built the document"
    );
    assert!(entered >= builds);
}
