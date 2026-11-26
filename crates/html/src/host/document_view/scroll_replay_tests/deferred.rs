//! A root scroll step replays a frame whose boxes paint in GPUI's deferred
//! pass (a positive `z-index`, a sticky box above the flow) next to a
//! viewport-fixed box: the deferred draws belong to the root scroll node
//! (`gpui::Deferred::in_scroll_node`) and move with it, so the step builds
//! nothing, and the frame, its paint order and its hit areas are those of a
//! fresh tree built at the new offset.

use super::*;

/// Rows with lifted boxes: `z-index` boxes with links overlapping the rows
/// around them, a sticky header with a positive `z-index` over its
/// section, and the viewport-fixed red box of `fixed_box`.
fn page() -> String {
    let style = "<style>body{margin:0}.r{height:20px}.r:nth-child(odd){background:#eee}\
        .z{position:relative;z-index:2;height:30px;width:120px;margin:-5px 0;background:#08f}\
        .z a{display:block;width:60px;height:12px;background:#0a0;cursor:pointer}\
        .s{position:sticky;top:0;z-index:3;height:24px;background:#f80;cursor:pointer}</style>";
    let section = |n: usize| {
        let rows: String = (0..60)
            .map(|i| match i % 9 {
                4 => format!("<div class=z><a href='#{n}-{i}'>{i}</a></div>"),
                _ => format!("<div class=r>row {n}-{i}</div>"),
            })
            .collect();
        format!("<section><div class=s>head {n}</div>{rows}</section>")
    };
    let sections: String = (0..6).map(section).collect();
    format!("{style}{}<main>{sections}</main>", fixed_box())
}

/// The hit areas a frame tests, in CSS px, sorted.
fn hit_areas(cx: &mut VisualTestContext) -> (Vec<String>, usize) {
    let (areas, draws) = cx.update(|window, _| window.painted_hit_areas());
    let mut areas: Vec<String> = areas
        .into_iter()
        .filter(|area| !area.is_empty())
        .map(|area| format!("{area:?}"))
        .collect();
    areas.sort();
    (areas, draws)
}

#[gpui::test]
fn wheel_steps_replay_deferred_draws_with_their_scroll_node(cx: &mut TestAppContext) {
    let html = page();
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    vcx.update(|_, cx| view.read(cx).document.take_frame_misses());
    assert!(hit_areas(vcx).1 >= 2, "the page paints deferred draws");
    // Down past a sticky header's section and back.
    for (i, dy) in [-120., -120., -120., -120., -120., 120., 120.]
        .into_iter()
        .enumerate()
    {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        let misses = vcx.update(|_, cx| view.read(cx).document.take_frame_misses());
        // The event's own frame replays the step, the next one replays still.
        assert_eq!((b1 - b0, r1 - r0), (0, 2), "{name}: built for {misses:?}");
        assert!(misses.is_empty(), "{name}: {misses:?}");
        assert_eq!(boxes(vcx), vec![(0., 100.)], "{name}: the fixed box moved");
        let offset = root(&view, vcx).offset();
        let (quads, areas) = fresh_with(cx, &html, offset, |c| (painted(c), hit_areas(c)));
        assert_same(&name, &painted(vcx), &quads);
        assert_eq!(hit_areas(vcx), areas, "{name}: hit areas");
    }
}

/// A positioned box with a `z-index` paints in a deferred draw, its blended
/// child in a composition group of that draw. The group spills over the
/// whole window (`group_area`), which a scroll does not move: a moved copy
/// would composite it off the window, so each wheel step builds
/// (`frame_miss.scroll_dependent`) and draws what a fresh tree at the new
/// offset draws.
#[gpui::test]
fn a_deferred_composition_group_spilling_over_the_window_builds_scrolled(cx: &mut TestAppContext) {
    let lifted = "<div style='position:relative;z-index:1;width:60px;height:60px;background:lime'>\
        <div style='mix-blend-mode:multiply;width:30px;height:30px;background:red'></div></div>";
    let html = rows(lifted);
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    frame(vcx);
    let labels = |c: &mut VisualTestContext| c.update(|window, _| window.painted_groups().0);
    for (i, dy) in [-20., -20., 20.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        let offset = root(&view, vcx).offset();
        assert_same(&name, &painted(vcx), &fresh_quads(cx, &html, offset));
        assert_eq!(labels(vcx), fresh_with(cx, &html, offset, labels), "{name}");
        let misses = vcx.update(|_, cx| view.read(cx).document.take_frame_misses());
        let dependent = misses.iter().any(|(miss, _)| miss == "scroll_dependent");
        assert!(
            dependent && b1 - b0 == 1,
            "{name}: builds {} for {misses:?}",
            b1 - b0
        );
    }
}

/// A hidden scroller registers its node but paints nothing as it: a root
/// step moves it with the root and replays, as a fresh tree draws.
#[gpui::test]
fn a_hidden_scroller_keeps_root_steps_replayed(cx: &mut TestAppContext) {
    let hidden = "<div style='visibility:hidden;overflow:auto;width:80px;height:30px'>\
        <div style='height:200px;background:red'></div></div>";
    let html = rows(hidden);
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    frame(vcx);
    for (i, dy) in [-20., -20., 20.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        let offset = root(&view, vcx).offset();
        assert_same(&name, &painted(vcx), &fresh_quads(cx, &html, offset));
        assert_eq!((b1 - b0, r1 - r0), (0, 2), "{name}");
    }
}
