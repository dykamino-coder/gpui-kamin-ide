//! A wheel over the root scrollport's bar scrolls the root, as a browser
//! scrolls the box whose scrollbar the pointer is over.

use super::fixed_tests::{root, rows};
use super::tests::{frame, open};
use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point};

#[gpui::test]
fn a_wheel_over_the_root_bar_lane_scrolls_the_root(cx: &mut TestAppContext) {
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &rows(""));
    // The centre of the classic vertical bar's lane at the 400px view's right edge.
    vcx.simulate_event(ScrollWheelEvent {
        position: point(px(392.5), px(150.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-100.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    frame(vcx);
    assert_eq!(root(&view, vcx).offset(), point(px(0.), px(-100.)));
}
