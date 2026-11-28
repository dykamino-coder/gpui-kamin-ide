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
fn text_paragraphs_are_kept_across_width_changes(cx: &mut TestAppContext) {
    // A width change breaks the lines of each paragraph again without building it:
    // the frame at every width equals a fresh document's (`compare`).
    let text = "alpha beta gamma delta ".repeat(12);
    let rows: String = (0..40).map(|i| format!("<p>{i} {text}</p>")).collect();
    let page = format!("<body style='margin:0'><div style='padding:1px'>{rows}</div></body>");
    let stats = compare(cx, &page, &[401., 300., 400.]);
    assert!(
        stats.recorded >= 40 && stats.resumed >= 2 * 40,
        "text paragraphs were built again: {stats:?}"
    );
}

#[gpui::test]
fn anonymous_runs_and_atom_content_are_kept_by_their_flow(cx: &mut TestAppContext) {
    // Runs between blocks have an anonymous box (no element of their own) and text inside
    // an inline-block lies one atom deep: both are kept and broken again at a new width.
    let text = "alpha beta gamma delta ".repeat(6);
    let rows: String = (0..20)
        .map(|i| {
            format!(
                "<div>{i} run {text}<p>block</p>{text} \
                 <span style='display:inline-block;width:120px'>{text}</span></div>"
            )
        })
        .collect();
    let page = format!("<body style='margin:0'>{rows}</body>");
    let stats = compare(cx, &page, &[401., 300., 400.]);
    let refused = |r: Refusal| stats.refused[r as usize];
    assert!(
        refused(Refusal::NoBox) == 0 && refused(Refusal::Clamp) == 0 && stats.resumed >= 60,
        "anonymous runs or atom content were built again: {stats:?}"
    );
}

#[gpui::test]
fn runs_that_start_with_an_inline_element_are_kept_by_their_flow(cx: &mut TestAppContext) {
    // A list of links with nested lists: the run of an item before its nested list starts
    // with an inline element, whose box (if any) does not name the run's flow; the run's
    // text does. The frame at each width equals a fresh document's.
    let text = "alpha beta gamma delta ".repeat(4);
    let items: String = (0..20)
        .map(|i| format!("<li><a href='#{i}'>{i} {text}</a> {text}<ol><li>{text}</li></ol></li>"))
        .collect();
    // Markers are left out: a marker run (generated text, no source) has no box of its own yet.
    let page = format!("<body style='margin:0;list-style:none'><ol>{items}</ol></body>");
    let stats = compare(cx, &page, &[401., 300., 400.]);
    assert!(
        stats.refused[Refusal::NoBox as usize] == 0 && stats.resumed >= 2 * 40,
        "runs were built again: {stats:?}"
    );
}

#[gpui::test]
fn paragraphs_with_generated_content_are_kept(cx: &mut TestAppContext) {
    // A pseudo-element names its originating element as its implicit anchor; in flow it
    // reads no other box, so its paragraph is kept like any other.
    let text = "alpha beta gamma delta ".repeat(10);
    let rows: String = (0..20).map(|i| format!("<p>{i} {text}</p>")).collect();
    let page = format!(
        "<style>p::before{{content:'\u{a7} '}} p::after{{content:' end'}}</style>\
         <body style='margin:0'>{rows}</body>"
    );
    let stats = compare(cx, &page, &[401., 300., 400.]);
    assert!(
        stats.refused[Refusal::Subtree as usize] == 0 && stats.resumed >= 2 * 20,
        "paragraphs with generated content were built again: {stats:?}"
    );
}

#[gpui::test]
fn text_in_shrink_to_fit_atoms_is_kept(cx: &mut TestAppContext) {
    // An inline-block of automatic width gets its constraint from the line; the build of
    // its paragraph reads no width, so a new constraint only breaks its lines again.
    let text = "alpha beta gamma delta ".repeat(8);
    let rows: String = (0..20)
        .map(|i| format!("<div>{i} <span style='display:inline-block'>{text}</span></div>"))
        .collect();
    let page = format!("<body style='margin:0'>{rows}</body>");
    let stats = compare(cx, &page, &[401., 300., 400.]);
    assert!(
        stats.resumed >= 2 * 20,
        "atom text was built again: {stats:?}"
    );
}

#[gpui::test]
fn every_recording_follows_a_named_miss(cx: &mut TestAppContext) {
    // Every record that does not give a frame its paragraph is counted by reason
    // (`paragraphs_miss_<reason>` in the journal).
    let text = "alpha beta gamma delta ".repeat(12);
    let rows: String = (0..10).map(|i| format!("<p>{i} {text}</p>")).collect();
    let page = format!("<body style='margin:0'>{rows}</body>");
    let stats = compare(cx, &page, &[401., 300.]);
    let missed: u64 = stats.missed.iter().sum();
    assert!(stats.resumed >= 10, "{stats:?}");
    // A record is made only by a build that took none: every recording after the first
    // frame's has a named miss.
    assert!(stats.recorded <= 10 + missed, "{stats:?}");
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
