//! A tree built while the root is scrolled draws what the tree built at
//! offset 0 draws, moved by the offset. A positioned box painted at its
//! source position (CSS 2.1 Appendix E: later positioned siblings) but laid
//! out in its containing block's layer takes the layer's scroll node, not the
//! window origin (`source_paint::split`). The absolute box below sits in a
//! plain block, so its containing block (the outer relative block) takes it
//! into its layer.

use super::*;

const SQUARE: &str = "width:50px;height:50px;background:#0a0";

/// Positioned rows after the box: its paint stays at its source position.
const LATER_POSITIONED: &str = "<style>.r:nth-child(9n){position:relative}</style>";

/// Content-space quads (CSS px, the root offset added back) whose top lies
/// in `[top, bottom)`: size, colour, origin.
fn content_quads(cx: &mut VisualTestContext, dy: f32, top: f32, bottom: f32) -> Vec<String> {
    let mut quads: Vec<String> = cx.update(|window, _| {
        let s = window.scale_factor();
        window
            .painted_quads()
            .into_iter()
            .filter_map(|q| {
                let t = q.transformation.translation;
                let (x, y) = (
                    (q.bounds.origin.x.0 + t[0]) / s,
                    (q.bounds.origin.y.0 + t[1]) / s - dy,
                );
                let (w, h) = (q.bounds.size.width.0 / s, q.bounds.size.height.0 / s);
                (y >= top && y + h <= bottom).then(|| format!("{x} {y} {w}x{h} {:?}", q.background))
            })
            .collect()
    });
    quads.sort();
    quads
}

#[gpui::test]
fn a_fixed_box_before_positioned_rows_stays_when_built_scrolled(cx: &mut TestAppContext) {
    let html = rows(&format!("{LATER_POSITIONED}{}", fixed_box()));
    for dy in [0., -120., -360.] {
        let at = fresh_with(cx, &html, point(px(0.), px(dy)), boxes);
        assert_eq!(at, vec![(0., 100.)], "built at {dy}");
    }
}

#[gpui::test]
fn an_absolute_box_painted_at_its_source_scrolls_with_its_block(cx: &mut TestAppContext) {
    let block = format!(
        "{LATER_POSITIONED}<div style='position:relative;height:300px'>\
         <div><div style='position:absolute;left:10px;top:200px;{SQUARE}'></div>\
         <div style='position:relative'>after</div></div></div>"
    );
    let html = rows(&block);
    let square = |cx: &mut VisualTestContext| -> Vec<(f32, f32)> {
        cx.update(|window, _| {
            let s = window.scale_factor();
            window
                .painted_quads()
                .into_iter()
                .filter(|q| (q.bounds.size.width.0 / s - 50.).abs() < 0.5)
                .map(|q| {
                    let t = q.transformation.translation;
                    (
                        (q.bounds.origin.x.0 + t[0]) / s,
                        (q.bounds.origin.y.0 + t[1]) / s,
                    )
                })
                .collect()
        })
    };
    for dy in [0., -40., -160.] {
        let at = fresh_with(cx, &html, point(px(0.), px(dy)), square);
        assert_eq!(at, vec![(10., 200. + dy)], "built at {dy}");
    }
}

#[gpui::test]
fn a_tree_built_scrolled_is_the_tree_built_at_zero_moved(cx: &mut TestAppContext) {
    let extra = format!(
        "{LATER_POSITIONED}{}<div style='position:relative;height:300px'>\
         <div><div style='position:absolute;left:10px;top:200px;{SQUARE}'></div>\
         <div style='position:relative'>after</div></div></div>",
        fixed_box()
    );
    let html = rows(&extra);
    // Content seen in both frames (a 300 px viewport) and clear of the fixed
    // box (content 100 at offset 0, 140 at 40): the absolute square at 200.
    let (top, bottom) = (195., 290.);
    let zero = fresh_with(cx, &html, point(px(0.), px(0.)), |cx| {
        content_quads(cx, 0., top, bottom)
    });
    assert!(!zero.is_empty());
    let moved = fresh_with(cx, &html, point(px(0.), px(-40.)), |cx| {
        content_quads(cx, -40., top, bottom)
    });
    assert_eq!(moved, zero);
}
