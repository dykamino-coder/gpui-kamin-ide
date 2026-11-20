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
