//! Blocks far from the screen stand as their exact leaf (`layout::reuse::offscreen`):
//! a frame scrolled anywhere draws what a fresh document scrolled there
//! without leaves draws, and the leaves keep nothing of the blocks.

use super::tests::{View, frame, open_document};
use crate::Document;
use crate::layout::reuse::offscreen_switch::set_off;
use gpui::{TestAppContext, VisualTestContext};

type Drawn = Vec<(String, [i32; 4])>;

fn scroller<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    open_document(
        cx,
        Document::builder(html, "").build_root_scroller().unwrap(),
    )
}

fn scroll_to(view: &gpui::Entity<View>, cx: &mut VisualTestContext, y: f32) {
    cx.update(|_, cx| assert!(view.read(cx).doc.set_root_scroll(y), "root scroller"));
    frame(cx);
}

/// The primitives of the last frame that show, in draw order.
fn drawn(cx: &mut VisualTestContext) -> Drawn {
    cx.update(|window, _| {
        let round = |v: f32| (v * 4.0).round() as i32;
        let shown = window.painted_sequence().into_iter();
        let shown = shown.filter(|(_, b)| b.size.width.0 > 0.0 && b.size.height.0 > 0.0);
        shown
            .map(|(kind, b)| {
                let (o, s) = (b.origin, b.size);
                (
                    kind,
                    [
                        round(o.x.0),
                        round(o.y.0),
                        round(s.width.0),
                        round(s.height.0),
                    ],
                )
            })
            .collect()
    })
}

fn leaves(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.offscreen_leaves())
}

/// What a fresh document without leaves draws scrolled to `y`.
fn fresh(cx: &mut TestAppContext, html: &str, y: f32) -> Drawn {
    set_off(true);
    let (view, cx) = scroller(cx, html);
    frame(cx);
    scroll_to(&view, cx, y);
    let drawn = drawn(cx);
    set_off(false);
    drawn
}

/// Paragraphs with collapsing margins, blocks whose children's margins meet
/// their edges, tables and multicol boxes, cut anywhere by the screen's edge.
fn plain_page() -> String {
    let text = "alpha beta gamma delta epsilon ".repeat(8);
    let blocks: String = (0..240)
        .map(|i| match i % 5 {
            0 => format!("<p style='margin:9px 0 5px;background:#eef'>{i} {text}</p>"),
            1 => format!(
                "<div style='background:#dfd'><p style='margin:7px 0'>{i} {text}</p>\
                 <p style='margin:3px 0'>{text}</p></div>"
            ),
            2 => format!(
                "<div style='padding:3px;background:#fdd'><table border=1>\
                 <tr><td>{i}</td><td>{text}</td></tr></table></div>"
            ),
            3 => format!("<div style='columns:2;background:#ffd'>{i} {text}{text}</div>"),
            _ => format!("<h2 style='margin:14px 0;background:#ccc'>Heading {i}</h2>"),
        })
        .collect();
    format!("<body style='margin:0'><div>{blocks}</div></body>")
}

/// A float escaping a far block, sticky and fixed boxes and a target of an
/// anchor in a far block, between plain paragraphs.
fn mixed_page() -> String {
    let text = "alpha beta gamma delta epsilon ".repeat(8);
    let blocks: String = (0..200)
        .map(|i| match i {
            60 => format!(
                "<div><span style='float:left;width:60px;height:400px;background:#f0f'></span>\
                 {text}</div>"
            ),
            90 => format!("<div style='position:sticky;top:0;background:#0ff'>{i} sticky</div>"),
            120 => "<div style='position:fixed;top:5px;left:5px;width:20px;height:20px;\
                    background:#00f'></div>"
                .into(),
            100 => format!(
                "<div style='margin:20px 0;background:#fcf'><div style='margin:30px 0'>\
                 <p style='margin:10px 0'>{i} {text}</p></div></div>\
                 <div style='margin:25px 0'></div>"
            ),
            150 => format!("<p id='target' style='background:#ff0'>{i} {text}</p>"),
            170 => "<div style='float:right;width:90px;height:900px;background:#f80'></div>".into(),
            _ => format!("<p style='margin:6px 0;background:#eef'>{i} {text}</p>"),
        })
        .collect();
    format!("<body style='margin:0'><div style='display:grid'><main>{blocks}</main></div></body>")
}

fn sweep(cx: &mut TestAppContext, html: &str, stops: &[f32]) -> u64 {
    let expected: Vec<Drawn> = stops.iter().map(|&y| fresh(cx, html, y)).collect();
    let (view, cx) = scroller(cx, html);
    frame(cx);
    for (y, expected) in stops.iter().zip(expected) {
        scroll_to(&view, cx, *y);
        assert_eq!(drawn(cx), expected, "the frame scrolled to {y} differs");
    }
    leaves(&view, cx)
}

#[gpui::test]
fn frames_scrolled_anywhere_draw_what_a_fresh_document_draws(cx: &mut TestAppContext) {
    let stops = [0.0, 2500.0, 9000.0, 21000.0, 4800.0, 15000.0, 0.0];
    let leaves = sweep(cx, &plain_page(), &stops);
    assert!(leaves > 100, "far blocks were built: {leaves} leaves");
}

#[gpui::test]
fn floats_sticky_and_fixed_boxes_draw_as_without_leaves(cx: &mut TestAppContext) {
    let stops = [
        0.0, 3000.0, 7600.0, 8000.0, 11400.0, 12700.0, 19000.0, 21600.0, 14000.0, 6000.0, 0.0,
    ];
    let leaves = sweep(cx, &mixed_page(), &stops);
    assert!(leaves > 50, "far blocks were built: {leaves} leaves");
}
