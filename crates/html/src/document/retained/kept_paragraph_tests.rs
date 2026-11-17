//! Paragraphs kept across a width change (`layout::reuse::paragraph`): the
//! lines are broken again without a build, the atoms that show are built,
//! and the frame equals a fresh document's.

use super::tests::{View, frame, open};
use crate::layout::reuse::paragraph::{KeepStats, Refusal};
use gpui::{ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px};

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

fn stats(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> KeepStats {
    cx.update(|_, cx| view.read(cx).doc.reuse.paragraphs.stats())
}

fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter();
        quads
            .map(|q| format!("{:?} {:?}", q.bounds, q.background))
            .collect()
    })
}

/// `count` atoms of `style` in sections without boxes: one paragraph.
fn atoms(count: usize, style: &str) -> String {
    let cell = format!(
        "<i style='display:inline-block;width:8px;height:8px;margin:1px;\
         background:#36c;border:1px solid #000{style}'></i>"
    );
    let section = format!(
        "<section style='display:contents'>{}</section>",
        cell.repeat(100)
    );
    format!(
        "<body style='margin:0'>{}</body>",
        section.repeat(count / 100)
    )
}

/// The page at each width equals a fresh document's; the stats at the end.
fn compare(cx: &mut TestAppContext, page: &str, widths: &[f32]) -> KeepStats {
    let (view, cx) = open(cx, page);
    frame(cx);
    for &width in widths {
        resize(&view, cx, width);
        let kept = quads(cx);
        let (fresh, fcx) = open(cx, page);
        resize(&fresh, fcx, width);
        assert_eq!(kept, quads(fcx), "width {width}");
    }
    stats(&view, cx)
}

#[gpui::test]
fn fixed_atoms_are_kept_across_width_changes(cx: &mut TestAppContext) {
    // 400 px holds 33 atoms a line: 61 lines, most below the window. At 800
    // the lines below come into the window: their atoms are built there.
    let stats = compare(cx, &atoms(2000, ""), &[401., 800., 400., 333., 400.]);
    assert!(
        stats.resumed >= 4,
        "the paragraph was built again: {stats:?}"
    );
}

#[gpui::test]
fn width_dependent_atoms_are_refused_by_name(cx: &mut TestAppContext) {
    for style in [";width:2%", ";margin-left:1%"] {
        let stats = compare(cx, &atoms(300, style), &[401., 400.]);
        assert_eq!(stats.resumed, 0, "{style}: {stats:?}");
        assert!(
            stats.refused[Refusal::AtomWidth as usize] > 0,
            "{style}: {stats:?}"
        );
    }
}

#[gpui::test]
fn text_paragraphs_are_kept(cx: &mut TestAppContext) {
    let text = "alpha beta gamma delta ".repeat(12);
    let rows: String = (0..40).map(|i| format!("<p>{i} {text}</p>")).collect();
    let page = format!("<body style='margin:0'><div style='padding:1px'>{rows}</div></body>");
    let stats = compare(cx, &page, &[401., 300., 400.]);
    assert!(
        stats.resumed > 40,
        "text paragraphs were built again: {stats:?}"
    );
}

/// Atoms in a scroll container: the atoms scrolled into the window after a
/// width change are painted as a fresh document paints them.
#[gpui::test]
fn atoms_scrolled_into_the_window_are_painted(cx: &mut TestAppContext) {
    let page = atoms(2000, "").replace(
        "<body style='margin:0'>",
        "<body style='margin:0'><div style='overflow:auto;height:300px'>",
    );
    let scroll = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-200.))),
            ..Default::default()
        });
        let _ = view;
        frame(cx);
    };
    let (view, cx) = open(cx, &page);
    frame(cx);
    resize(&view, cx, 333.);
    resize(&view, cx, 401.);
    scroll(&view, cx);
    let kept = quads(cx);
    let (fresh, fcx) = open(cx, &page);
    resize(&fresh, fcx, 401.);
    scroll(&fresh, fcx);
    assert_eq!(kept, quads(fcx));
    assert!(stats(&view, cx).resumed > 0, "{:?}", stats(&view, cx));
}
