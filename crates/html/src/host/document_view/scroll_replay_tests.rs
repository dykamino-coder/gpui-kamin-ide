//! A root scroll step inside the recorded interest rect replays the frame
//! moved (`gpui::Window::plan_scrolled_replay`): no build, and the painted
//! quads are those of a fresh tree built at the new offset.

use super::fixed_tests::{boxes, fixed_box, root, rows};
use super::paint_compare::{painted, same_paint};

type Painted = (String, u32, gpui::Bounds<gpui::ScaledPixels>);
use super::tests::{frame, open};
use super::*;
use gpui::{
    Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
};

fn wheel_by(cx: &mut VisualTestContext, dy: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(200.), px(20.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
}

fn frames(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).document.retained_frames())
}

/// The quads of a new tree built at `offset`, not a replay.
fn fresh_quads(cx: &mut TestAppContext, html: &str, offset: gpui::Point<Pixels>) -> Vec<Painted> {
    fresh_with(cx, html, offset, painted)
}

/// What `read` reads from the frame of a new tree built at `offset`.
fn fresh_with<T>(
    cx: &mut TestAppContext,
    html: &str,
    offset: gpui::Point<Pixels>,
    read: impl FnOnce(&mut VisualTestContext) -> T,
) -> T {
    let mut app = cx.clone();
    let (view, fresh) = open(&mut app, html);
    root(&view, fresh).set_offset(offset);
    fresh.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document
                .update(&format!("{html}<!-- fresh -->"), "")
                .unwrap();
            cx.notify();
        })
    });
    frame(fresh);
    frame(fresh);
    assert_eq!(root(&view, fresh).offset(), offset);
    read(fresh)
}

fn assert_same(step: &str, after: &[Painted], fresh: &[Painted]) {
    if let Err(diff) = same_paint(after, fresh) {
        panic!("{step}: {diff}");
    }
}

/// One step: `input` moves the root, one frame paints it. Returns the
/// builds and replays the step cost, after checking it against a fresh frame.
fn step(
    cx: &mut TestAppContext,
    html: &str,
    name: &str,
    input: impl FnOnce(&mut VisualTestContext),
    view: &Entity<DocumentView>,
    vcx: &mut VisualTestContext,
) -> (u64, u64) {
    let before = root(view, vcx).offset();
    let (b0, r0) = frames(view, vcx);
    input(vcx);
    frame(vcx);
    let (b1, r1) = frames(view, vcx);
    let offset = root(view, vcx).offset();
    assert_ne!(offset, before, "{name}: the root did not scroll");
    assert_eq!(boxes(vcx), vec![(0., 100.)], "{name}: the fixed box moved");
    let after = painted(vcx);
    let name = format!("{name} (builds {}, replays {})", b1 - b0, r1 - r0);
    assert_same(&name, &after, &fresh_quads(cx, html, offset));
    (b1 - b0, r1 - r0)
}

#[gpui::test]
fn wheel_steps_inside_the_interest_rect_replay_without_a_build(cx: &mut TestAppContext) {
    let html = rows(&fixed_box());
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -120., -120., -120., 120., 120.]
        .into_iter()
        .enumerate()
    {
        let name = format!("wheel {i} by {dy}");
        // The event's own frame replays the step, the next one replays still.
        let (builds, replays) = step(cx, &html, &name, |c| wheel_by(c, dy), &view, vcx);
        assert_eq!((builds, replays), (0, 2), "{name}");
    }
}

/// The root element's decorations paint stationary in the root scrollport
/// (`scroll_target::stationary`, a scroll anchor): a step still replays.
#[gpui::test]
fn a_stationary_root_background_keeps_steps_replayed(cx: &mut TestAppContext) {
    let html = format!(
        "<style>html{{background:#fafafa}}</style>{}",
        rows(&fixed_box())
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (builds, replays) = step(cx, &html, &name, |c| wheel_by(c, dy), &view, vcx);
        assert_eq!((builds, replays), (0, 2), "{name}");
    }
}

#[gpui::test]
fn scroll_keys_replay_near_steps_and_build_far_jumps(cx: &mut TestAppContext) {
    let html = rows(&fixed_box());
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for key in ["pagedown", "down", "pageup"] {
        // A key may also draw before its handlers run (`Window::dispatch_key_event`).
        let (builds, replays) = step(cx, &html, key, |c| c.simulate_keystrokes(key), &view, vcx);
        assert_eq!(builds, 0, "{key} (replays {replays})");
        assert!(replays > 0, "{key}");
    }
    // End and Home leave the recorded rect (8000 px of rows): a fresh build,
    // still the same frame as a new tree at that offset.
    for key in ["end", "pageup", "home"] {
        step(cx, &html, key, |c| c.simulate_keystrokes(key), &view, vcx);
    }
}

#[gpui::test]
fn scripted_viewport_scroll_matches_fresh_and_then_replays(cx: &mut TestAppContext) {
    let html = rows(&fixed_box());
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    let inside = |vcx: &mut VisualTestContext| {
        vcx.update(|_, cx| view.read(cx).document.builds_inside_draw())
    };
    let initial_inside = inside(vcx);
    for (y, expected_builds) in [(-170., 0), (-6500., 1)] {
        let before = frames(&view, vcx);
        let offset = point(px(0.), px(y));
        root(&view, vcx).set_offset(offset);
        frame(vcx);
        assert_eq!(root(&view, vcx).offset(), offset);
        let reference = fresh_quads(cx, &html, offset);
        assert_same("scripted scroll", &painted(vcx), &reference);
        let moved = frames(&view, vcx);
        assert_eq!(
            moved,
            (before.0 + expected_builds, before.1 + 1 - expected_builds)
        );
        assert_eq!(
            inside(vcx),
            initial_inside,
            "a far jump must be prepared before draw"
        );
        frame(vcx);
        assert_eq!(frames(&view, vcx), (moved.0, moved.1 + 1));
        assert_same("still after scroll", &painted(vcx), &reference);
    }
}

mod canvas;
mod groups;

/// Block ranges from the previous layer must not survive the moved scene.
#[gpui::test]
fn moving_a_child_after_a_scrolled_replay_matches_fresh(cx: &mut TestAppContext) {
    use crate::document::Mutation;
    let css = "<style>body{margin:0}.r{height:20px;background:#ddd}#probe{height:45px;background:green}#flag{height:35px;background:red}</style>";
    let row = "<div class=r></div>";
    let probe = "<div id=probe></div>";
    let flag = "<div id=flag></div>";
    let lead = row.repeat(9);
    let rest = row.repeat(90);
    let original = format!("{css}<body>{lead}{probe}{rest}{flag}</body>");
    let moved = format!("{css}<body>{lead}{flag}{probe}{rest}</body>");
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &original);
    let offset = point(px(0.), px(-120.));
    let (builds, replays) = frames(&view, vcx);
    root(&view, vcx).set_offset(offset);
    frame(vcx);
    assert_eq!(frames(&view, vcx), (builds, replays + 1));
    assert_same(
        "before move",
        &painted(vcx),
        &fresh_quads(cx, &original, offset),
    );
    let reference = fresh_quads(cx, &moved, offset);
    assert!(same_paint(&painted(vcx), &reference).is_err());
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            let doc = view.document_mut();
            let element = |id: &str| {
                let attr = format!("id={id}");
                let records = doc
                    .mutations(&original.replace(&attr, &format!("{attr} data-probe")))
                    .unwrap();
                let [Mutation::Attribute { element, .. }] = &records[..] else {
                    panic!("expected one source element for {id}");
                };
                element.clone()
            };
            let (child, before) = (element("flag"), element("probe"));
            let link = before.parent.take().unwrap();
            let parent = link.upgrade().unwrap();
            before.parent.set(Some(link));
            let record = Mutation::InsertBefore {
                parent: doc.source_node(&parent).unwrap(),
                child: doc.source_node(&child).unwrap(),
                before: Some(before),
            };
            doc.apply(&[record], "").unwrap();
            cx.notify();
        });
    });
    frame(vcx);
    assert_eq!(root(&view, vcx).offset(), offset);
    assert_same("moved child", &painted(vcx), &reference);
    let (builds, replays) = frames(&view, vcx);
    frame(vcx);
    assert_eq!(frames(&view, vcx), (builds, replays + 1));
    assert_same("replayed child", &painted(vcx), &reference);
}
