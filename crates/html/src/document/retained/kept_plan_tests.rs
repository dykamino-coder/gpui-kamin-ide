//! Prepared child lists of block containers kept across a width change
//! (`layout::reuse::child_plan`): a new width prepares only the containers whose
//! preparation read the width, a mutation prepares its containers again, and every
//! frame equals a fresh document's.

use super::reuse_tests::update;
use super::tests::{View, frame, open};
use crate::layout::reuse::child_plan::PlanStats;
use gpui::{TestAppContext, VisualTestContext};

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

fn stats(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> PlanStats {
    cx.update(|_, cx| view.read(cx).doc.reuse.child_plans.stats())
}

fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter();
        quads
            .map(|q| format!("{:?} {:?}", q.bounds, q.background))
            .collect()
    })
}

fn same_as_fresh(cx: &mut VisualTestContext, page: &str, width: f32) {
    let kept = quads(cx);
    let (fresh, fcx) = open(cx, page);
    resize(&fresh, fcx, width);
    assert_eq!(kept, quads(fcx), "width {width}");
}

/// Sections of nested blocks with collapsing margins and text; every fifth holds a
/// float, whose host reads the width of its containing block.
fn page(marked: Option<usize>) -> String {
    let text = "alpha beta gamma delta ".repeat(5);
    let rows: String = (0..30)
        .map(|i| {
            let float = match i % 5 {
                0 => "<div style='float:left;width:40px;height:20px;background:#c63'></div>",
                _ => "",
            };
            let border = match marked == Some(i) {
                true => "border-top:3px solid #000",
                false => "border-top:1px solid #36c",
            };
            format!(
                "<section style='{border}'><div style='margin:6px 0'>{float}<p>{i} {text}</p>\
                 <div style='margin:4px 0;background:#eee'><p>{text}</p></div></div></section>"
            )
        })
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

#[gpui::test]
fn block_children_are_prepared_once_across_width_changes(cx: &mut TestAppContext) {
    let page = page(None);
    let (view, cx) = open(cx, &page);
    frame(cx);
    let first = stats(&view, cx);
    for width in [401., 300., 400.] {
        resize(&view, cx, width);
        same_as_fresh(cx, &page, width);
    }
    let after = stats(&view, cx);
    let built = after.built - first.built;
    assert!(
        first.kept >= 60 && after.reused - first.reused >= 3 * 60 && built * 3 <= first.built,
        "block containers were prepared again at a new width: {first:?} -> {after:?}"
    );
}

#[gpui::test]
fn a_mutation_prepares_its_containers_again(cx: &mut TestAppContext) {
    // A script changes one section's style: its container and those above it read the
    // changed box (their build epochs move); the frame equals a fresh document's.
    let (view, cx) = open(cx, &page(None));
    frame(cx);
    resize(&view, cx, 400.);
    let before = stats(&view, cx);
    let changed = page(Some(7));
    update(&view, cx, &changed);
    resize(&view, cx, 400.);
    same_as_fresh(cx, &changed, 400.);
    let after = stats(&view, cx);
    assert!(
        after.built > before.built && after.reused > before.reused,
        "a mutation was not prepared again, or nothing else was kept: {before:?} -> {after:?}"
    );
}
