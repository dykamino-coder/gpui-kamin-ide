//! A root scroll step replays a frame that holds composition groups (opacity,
//! filter, blend mode, mask) with the groups moved (`gpui::Window::replay_groups`):
//! no build, and the frame and every group draw what a fresh tree built at the
//! new offset draws.

use super::*;

/// Rows with composition groups in the root flow: opacity, filter, blend
/// mode and mask, each on a row with a background and text.
const STYLE: &str = "<style>.r:nth-child(7n){opacity:.5}.r:nth-child(11n){filter:blur(1px)}\
    .r:nth-child(13n){mix-blend-mode:multiply}\
    .r:nth-child(17n){mask-image:linear-gradient(#000,transparent)}</style>";

type Groups = (Vec<gpui::Bounds<gpui::ScaledPixels>>, Vec<String>);

/// The drawn areas of the group labels and each group that draws with its
/// bounds and drawn quads (translation folded into the bounds, orders
/// dropped), sorted.
fn groups(cx: &mut VisualTestContext) -> Groups {
    let (labels, groups) = cx.update(|window, _| window.painted_groups());
    let mut areas: Vec<_> = labels.into_iter().map(|(_, area)| area).collect();
    areas.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    // A group of a label out of view draws nothing; how many such groups a
    // frame keeps depends on where it was recorded.
    let mut groups: Vec<String> = groups
        .into_iter()
        .filter(|(_, quads)| !quads.is_empty())
        .map(|(bounds, quads)| {
            let mut quads: Vec<String> = quads
                .into_iter()
                .map(|mut q| {
                    q.order = 0;
                    let t = &mut q.transformation;
                    q.bounds.origin.x.0 += t.translation[0];
                    q.bounds.origin.y.0 += t.translation[1];
                    t.translation = [0., 0.];
                    format!("{q:?}")
                })
                .collect();
            quads.sort();
            format!("{bounds:?} {quads:?}")
        })
        .collect();
    groups.sort();
    (areas, groups)
}

#[gpui::test]
fn wheel_steps_replay_a_frame_with_composition_groups_without_a_build(cx: &mut TestAppContext) {
    let html = rows(STYLE);
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    let painted_groups = groups(vcx).1.len();
    assert!(
        painted_groups >= 2,
        "the fixture paints composition groups: {painted_groups}"
    );
    for (i, dy) in [-120., -120., -120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        // The event's own frame replays the step, the next one replays still.
        let misses = vcx.update(|_, cx| view.read(cx).document.take_frame_misses());
        assert_eq!((b1 - b0, r1 - r0), (0, 2), "{name}: built for {misses:?}");
        let offset = root(&view, vcx).offset();
        let (quads, reference) = fresh_with(cx, &html, offset, |c| (painted(c), groups(c)));
        assert_same(&name, &painted(vcx), &quads);
        assert_eq!(groups(vcx), reference, "{name}: groups");
    }
}

/// The decoration of an empty filtered box (a conic gradient, a child
/// element) gives way to a plain fill on a page that scrolls; then wheel
/// steps draw what a fresh tree at each offset draws. (Whether the edit is
/// taken in place is `effects_repaint_tests`' check: here the restyle plan
/// refuses it, `refused_plan_text`.)
#[gpui::test]
fn wheel_steps_after_a_decoration_edit_of_a_group_root_match_fresh(cx: &mut TestAppContext) {
    let style = format!(
        "{STYLE}<style>.fx{{height:40px;filter:blur(1px);\
         background:repeating-conic-gradient(from 13deg,#e457,#37e8 10deg,#fc98 30deg)}}\
         .fx[data-edit]{{background:#eee}}</style>"
    );
    let html = rows(&style).replace(
        "<div class=r>row 13</div>",
        "<div class=r>row 13</div><div class=fx></div>",
    );
    let edited = html.replace("<div class=fx>", "<div class=fx data-edit>");
    assert_ne!(html, edited, "the fixture has the filtered box");
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document.update(&edited, "").unwrap();
            cx.notify();
        })
    });
    frame(vcx);
    for (i, dy) in [0., -120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        if dy != 0. {
            wheel_by(vcx, dy);
            frame(vcx);
        }
        let offset = root(&view, vcx).offset();
        let (quads, reference) = fresh_with(cx, &edited, offset, |c| (painted(c), groups(c)));
        assert_same(&name, &painted(vcx), &quads);
        assert_eq!(groups(vcx), reference, "{name}: groups");
    }
}

/// A colour edit inside an opacity group first (the document's layer may
/// keep its groups, `gpui::Window::paint_grouped_layer`), then wheel steps:
/// every scrolled frame still draws what a fresh tree at that offset draws.
#[gpui::test]
fn wheel_steps_after_an_edit_inside_a_group_match_fresh(cx: &mut TestAppContext) {
    let style = format!("{STYLE}<style>[data-edit]{{background:#e45}}</style>");
    let html = rows(&style);
    let edited = html.replace(
        "<div class=r>row 13</div>",
        "<div class=r data-edit>row 13</div>",
    );
    assert_ne!(html, edited, "the fixture has row 13");
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document.update(&edited, "").unwrap();
            cx.notify();
        })
    });
    frame(vcx);
    let offset = root(&view, vcx).offset();
    let (quads, reference) = fresh_with(cx, &edited, offset, |c| (painted(c), groups(c)));
    assert_same("edited", &painted(vcx), &quads);
    assert_eq!(groups(vcx), reference, "edited: groups");
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        wheel_by(vcx, dy);
        frame(vcx);
        let offset = root(&view, vcx).offset();
        let (quads, reference) = fresh_with(cx, &edited, offset, |c| (painted(c), groups(c)));
        assert_same(&name, &painted(vcx), &quads);
        assert_eq!(groups(vcx), reference, "{name}: groups");
        frame(vcx);
        assert_same(&format!("{name} still"), &painted(vcx), &quads);
        assert_eq!(groups(vcx), reference, "{name} still: groups");
    }
}

/// Wheel steps first (the frame scrolls its copy of the document's framed
/// layer and the layer is left, `Store::leave_framed_layer`), then a colour
/// edit inside a row: the frame draws what a fresh tree at that offset draws.
#[gpui::test]
fn an_edit_after_wheel_steps_matches_fresh(cx: &mut TestAppContext) {
    let style = format!("{STYLE}<style>[data-edit]{{background:#e45}}</style>");
    let html = rows(&style);
    let edited = html.replace(
        "<div class=r>row 13</div>",
        "<div class=r data-edit>row 13</div>",
    );
    assert_ne!(html, edited, "the fixture has row 13");
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for dy in [-120., -120.] {
        wheel_by(vcx, dy);
        frame(vcx);
    }
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document.update(&edited, "").unwrap();
            cx.notify();
        })
    });
    frame(vcx);
    let offset = root(&view, vcx).offset();
    let (quads, reference) = fresh_with(cx, &edited, offset, |c| (painted(c), groups(c)));
    assert_same("edited", &painted(vcx), &quads);
    assert_eq!(groups(vcx), reference, "edited: groups");
}
