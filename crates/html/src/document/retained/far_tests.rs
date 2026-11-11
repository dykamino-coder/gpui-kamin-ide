//! Far blocks laid out from their kept layout subtree after a width change
//! (`layout::reuse::far`): no build, and the frame equals a fresh document's.

use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

fn far(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.far_blocks())
}

fn rebuilt(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.reused_blocks().2)
}

/// Wrapping paragraphs; `shift` moves the flow up so that the first ones
/// lie far above the screen and the visible ones stand on their heights.
fn paragraphs(shift: u32) -> String {
    let text = "alpha beta gamma delta ".repeat(12);
    let rows: String = (0..300)
        .map(|i| format!("<p style='margin:0 0 3px;background:#eee'>{i} {text}</p>"))
        .collect();
    format!("<body style='margin:0;margin-top:-{shift}px'>{rows}</body>")
}

fn quads(cx: &mut VisualTestContext) -> Vec<(f32, f32, f32, f32)> {
    cx.update(|window, _| {
        window
            .painted_quads()
            .iter()
            .map(|q| {
                let b = q.bounds;
                (b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0)
            })
            .collect()
    })
}

#[gpui::test]
fn far_paragraphs_are_laid_out_without_a_build_after_a_width_change(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &paragraphs(0));
    frame(cx);
    resize(&view, cx, 401.);
    assert_eq!(far(&view, cx), 0, "the first build at a new width records");
    resize(&view, cx, 400.);
    let first = far(&view, cx);
    assert!(first > 200, "far paragraphs were built: {first}");
    resize(&view, cx, 399.);
    assert!(far(&view, cx) - first > 200, "far records were not kept");
    assert_eq!(rebuilt(&view, cx), 0, "a far block was refused");
}

/// Paragraphs far above the screen decide where the visible ones stand: the
/// frame at each width equals a fresh document's at that width.
#[gpui::test]
fn blocks_below_far_paragraphs_stand_where_a_fresh_build_puts_them(cx: &mut TestAppContext) {
    let page = paragraphs(6000);
    let (view, cx) = open(cx, &page);
    frame(cx);
    for width in [401., 400., 397., 400.] {
        resize(&view, cx, width);
        let reused = quads(cx);
        let (fresh, fcx) = open(cx, &page);
        resize(&fresh, fcx, width);
        assert_eq!(reused, quads(fcx), "width {width}");
    }
    assert!(far(&view, cx) > 100, "the paragraphs above were built");
    assert_eq!(rebuilt(&view, cx), 0);
}
