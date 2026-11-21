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
