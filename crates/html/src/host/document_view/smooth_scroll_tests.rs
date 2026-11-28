//! Controlled-vsync scrolling: native input, CSSOM, cancellation and frame work.

use super::fixed_tests::root;
use super::tests::{LONG, open};
use super::*;
use crate::document::ScrollBehavior;
use gpui::{
    AnimationElapsedTime, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, point,
};
use std::time::Duration;

fn at(cx: &mut VisualTestContext, ms: u64) {
    cx.update(|window, cx| {
        cx.set_global(AnimationElapsedTime(Duration::from_millis(ms)));
        window.simulate_next_frame(cx);
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn wheel(cx: &mut VisualTestContext, precise: bool) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(200.), px(20.)),
        delta: if precise {
            ScrollDelta::Pixels(point(px(0.), px(-30.)))
        } else {
            ScrollDelta::Lines(point(0., -3.))
        },
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
}

fn request(
    view: &gpui::Entity<DocumentView>,
    cx: &mut VisualTestContext,
    y: f32,
    behavior: ScrollBehavior,
) {
    cx.update(|window, cx| {
        view.read(cx)
            .document
            .scroll_to(0., y, behavior, window, cx);
    });
}

#[gpui::test]
fn discrete_wheel_animates_and_precise_input_cancels(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    at(cx, 0);
    let handle = root(&view, cx);
    wheel(cx, false);
    assert_eq!(
        handle.offset().y,
        px(0.),
        "discrete wheel must change only the goal"
    );
    at(cx, 50);
    assert!(handle.offset().y < px(0.));
    let before = handle.offset();
    wheel(cx, true);
    assert_eq!(handle.offset().y, before.y - px(30.));
    let stopped = handle.offset();
    at(cx, 500);
    assert_eq!(handle.offset(), stopped);
}

#[gpui::test]
fn css_auto_instant_override_and_reduced_motion(cx: &mut TestAppContext) {
    let html = format!("{LONG}<style>html{{scroll-behavior:smooth}}</style>");
    let (view, cx) = open(cx, &html);
    at(cx, 0);
    let handle = root(&view, cx);
    request(&view, cx, 200., ScrollBehavior::Auto);
    assert_eq!(handle.offset().y, px(0.));
    at(cx, 50);
    assert!(handle.offset().y < px(0.) && handle.offset().y > px(-200.));
    request(&view, cx, 200., ScrollBehavior::Instant);
    assert_eq!(handle.offset().y, px(-200.));
    at(cx, 500);
    assert_eq!(handle.offset().y, px(-200.));
    cx.update(|_, cx| cx.set_reduce_motion(true));
    request(&view, cx, 400., ScrollBehavior::Smooth);
    assert_eq!(handle.offset().y, px(-400.));
}

#[gpui::test]
fn every_vsync_is_a_retained_translation_with_journal_counts(cx: &mut TestAppContext) {
    let html = format!(
        "{LONG}<style>html{{background:#fafafa}}aside{{position:fixed;top:10px;left:10px;width:40px;height:40px;background:red}}</style><aside></aside>"
    );
    let (view, cx) = open(cx, &html);
    at(cx, 0);
    let (_guard, _reader) = crate::why::test_reader();
    let before = cx.update(|_, cx| view.read(cx).document.retained_frames());
    request(&view, cx, 500., ScrollBehavior::Smooth);
    for ms in (16..=384).step_by(16) {
        at(cx, ms);
    }
    let after = cx.update(|_, cx| view.read(cx).document.retained_frames());
    if after.0 != before.0 {
        panic!("animation rebuilt the document: {:?}", crate::why::take());
    }
    assert_eq!(root(&view, cx).offset().y, px(-500.));
    let entry = crate::why::take();
    assert!(entry.journal.scroll_animation_frames > 1);
    assert_eq!(entry.journal.scroll_animation_builds, 0);
    assert_eq!(crate::why::to_json(&entry)["scroll_animation"]["builds"], 0);
}

#[gpui::test]
fn keys_and_track_use_the_same_animator(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    at(cx, 0);
    cx.simulate_keystrokes("pagedown");
    let handle = root(&view, cx);
    assert_eq!(handle.offset().y, px(0.));
    at(cx, 50);
    assert!(handle.offset().y < px(0.) && handle.offset().y > px(-200.));
    at(cx, 200);
    assert!(handle.offset().y < px(-200.));
    request(&view, cx, 0., ScrollBehavior::Instant);
    at(cx, 210);
    cx.simulate_event(gpui::MouseDownEvent {
        position: point(px(392.5), px(200.)),
        button: gpui::MouseButton::Left,
        ..Default::default()
    });
    assert_eq!(handle.offset().y, px(0.));
    at(cx, 260);
    assert!(handle.offset().y < px(0.) && handle.offset().y > px(-200.));
    at(cx, 410);
    assert!(handle.offset().y < px(-200.));
}

#[gpui::test]
fn geometry_query_samples_between_vsyncs_and_relative_scroll_uses_it(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    at(cx, 0);
    request(&view, cx, 500., ScrollBehavior::Smooth);
    let before = cx.update(|_, cx| view.read(cx).document.retained_frames());
    cx.update(|window, cx| {
        cx.set_global(AnimationElapsedTime(Duration::from_millis(50)));
        let document = &view.read(cx).document;
        assert_eq!(document.root_scroll_offset(), Some((0., 0.)));
        let (_, top) = document.current_root_scroll_offset(window, cx).unwrap();
        assert!(top > 0. && top < 500.);
        document.scroll_by(0., 30., ScrollBehavior::Instant, window, cx);
        assert_eq!(document.root_scroll_offset(), Some((0., top + 30.)));
        assert_eq!(document.retained_frames(), before);
    });
    let offset = root(&view, cx).offset();
    at(cx, 500);
    assert_eq!(root(&view, cx).offset(), offset);
}
