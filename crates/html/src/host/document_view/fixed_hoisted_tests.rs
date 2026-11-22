//! Out-of-flow boxes hoisted to the root ICB layer from a subtree laid out
//! ahead of the layer (a float, generated content): they follow the root
//! scroll like the layer itself, so an absolute box scrolls and a fixed box
//! stays on the viewport, offset once.

use super::fixed_tests::{RED, boxes, fixed_box, matches_fresh, root, rows, wheel};
use super::tests::{frame, open};
use gpui::TestAppContext;

fn pseudo(host: &str) -> String {
    format!(
        "<style>.p:before,.p:after{{content:'';position:fixed;top:100px;left:0;{RED}}}\
         .p:after{{left:100px}}</style>{host}"
    )
}

/// Generated fixed boxes of a block and of a float (css-zen-garden's aside).
fn hosts() -> [String; 3] {
    [
        pseudo("<div class=p></div>"),
        pseudo("<aside class=p style='float:left;width:100%'></aside>"),
        format!(
            "<div style='float:left;width:100%'>text{}</div>",
            fixed_box()
        ),
    ]
}

#[gpui::test]
fn fixed_generated_and_float_boxes_stay_while_the_root_scrolls(cx: &mut TestAppContext) {
    for extra in hosts() {
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &rows(&extra));
        let before = boxes(vcx);
        wheel(vcx);
        frame(vcx);
        assert!(root(&view, vcx).offset().y < gpui::px(0.));
        assert_eq!(boxes(vcx), before, "wheel: {extra}");
        vcx.simulate_keystrokes("end");
        frame(vcx);
        assert_eq!(boxes(vcx), before, "end: {extra}");
    }
}

#[gpui::test]
fn an_absolute_box_of_a_float_scrolls_with_the_content(cx: &mut TestAppContext) {
    let extra = format!(
        "<div style='float:left;width:100%'>\
         <div style='position:absolute;top:100px;left:0;{RED}'></div></div>"
    );
    let (view, cx) = open(cx, &rows(&extra));
    wheel(cx);
    frame(cx);
    let dy = f32::from(root(&view, cx).offset().y);
    assert!(dy < 0.);
    assert_eq!(boxes(cx), vec![(0., 100. + dy)]);
}

#[gpui::test]
fn a_scrolled_frame_with_hoisted_fixed_boxes_matches_a_fresh_one(cx: &mut TestAppContext) {
    for extra in hosts() {
        matches_fresh(cx, &rows(&extra));
    }
}

#[gpui::test]
fn hoisted_fixed_and_absolute_boxes_share_positive_z_order(cx: &mut TestAppContext) {
    for fixed_z in [1, i32::MAX] {
        for absolute_z in [0, 2] {
            for fixed_first in [false, true] {
                let fixed = format!(
                    "<div style='position:fixed;top:0;left:0;width:32px;height:32px;\
                     background:red;z-index:{fixed_z}'></div>"
                );
                let absolute = format!(
                    "<header style='position:absolute;top:0;left:0;right:0;height:1000px;\
                     background:blue;z-index:{absolute_z}'></header>"
                );
                let siblings = if fixed_first {
                    format!("{fixed}{absolute}")
                } else {
                    format!("{absolute}{fixed}")
                };
                let html = format!(
                    "<style>body{{margin:0;display:flex;flex-direction:column;min-height:100vh}}\
                     </style>{siblings}<main style='height:1800px'></main>"
                );
                let mut app = cx.clone();
                let (view, vcx) = open(&mut app, &html);
                for scrolled in [false, true] {
                    if scrolled {
                        wheel(vcx);
                        frame(vcx);
                        assert!(root(&view, vcx).offset().y < gpui::px(0.));
                    }
                    vcx.update(|window, _| {
                        let quads = window.painted_quads();
                        let fixed = quads
                            .iter()
                            .find(|q| q.background.as_solid() == Some(gpui::red()))
                            .unwrap();
                        assert_eq!(
                            (
                                (fixed.bounds.origin.x.0 + fixed.transformation.translation[0])
                                    / window.scale_factor(),
                                (fixed.bounds.origin.y.0 + fixed.transformation.translation[1])
                                    / window.scale_factor(),
                            ),
                            (0., 0.),
                            "the fixed viewport anchor survives z-order deferral"
                        );
                        let order = |color| {
                            quads
                                .iter()
                                .find(|q| q.background.as_solid() == Some(color))
                                .unwrap()
                                .order
                        };
                        assert_eq!(
                            order(gpui::red()) > order(gpui::blue()),
                            fixed_z > absolute_z,
                            "fixed={fixed_z}, absolute={absolute_z}, first={fixed_first}, scrolled={scrolled}"
                        );
                    });
                }
            }
        }
    }
}
