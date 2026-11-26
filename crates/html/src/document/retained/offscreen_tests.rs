//! Paint-only edits wholly off the screen of boxes the scene layer cannot
//! record (`revision::offscreen`): a band on a justified line or on a nested
//! inline box, a box with decorations. The frame is kept and the edit waits;
//! its frame, and the frames that bring it onto the screen (a taller window,
//! a taller viewport), equal a fresh document's. A box whose paint reaches
//! the screen, or whose fragments no longer describe the layer, is never
//! left waiting.

use crate::document::retained::box_repaint_tests::{frames, quads};
use crate::document::retained::reuse_tests::{settled, update};
use crate::document::retained::tests::{View, frame, open};
use crate::dom::Node;
use gpui::{TestAppContext, VisualTestContext, px, size};

const WORDS: &str = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do ";

/// One justified paragraph of 200 spans; `edit` goes into the start tag of
/// span `at`, `inner` wraps its text in a nested span.
fn spans(at: usize, edit: &str, inner: bool, css: &str) -> String {
    let body: String = (0..200)
        .map(|i| match (i == at, inner) {
            (true, true) => format!("<span id=o{i}><span id=s{i}{edit}>{WORDS}</span></span>"),
            (true, false) => format!("<span id=s{i}{edit}>{WORDS}</span>"),
            _ => format!("<span>{WORDS}</span>"),
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         p{{width:380px;font:16px/20px sans-serif;margin:0;text-align:justify}}\
         [data-edit]{{background:#eee}} {css}</style></head><body><p>{body}</p></body></html>"
    )
}

/// Rows of 20px with a shadow; `edit` goes into the start tag of row `at`,
/// whose own shadow is `shadow`.
fn rows(at: usize, edit: &str, shadow: &str) -> String {
    let body: String = (0..200)
        .map(|i| match i == at {
            true => format!("<div class=r style='box-shadow:{shadow}'{edit}>row {i}</div>"),
            false => format!("<div class=r>row {i}</div>"),
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         .r{{height:20px;box-shadow:0 0 0 1px #888}} [data-edit]{{background:#eee}}\
         </style></head><body>{body}</body></html>"
    )
}

fn window(cx: &mut VisualTestContext, height: f32) {
    cx.simulate_resize(size(px(400.), px(height)));
    frame(cx);
}

fn tall(view: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4200.));
    frame(cx);
}

fn waiting(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| view.read(cx).doc.reuse.deferred_frames())
}

/// The edit of `page("")` to `page(" data-edit")` from a settled frame in a
/// 300px window: whether it kept the frame, the edits waiting, the scene
/// after it and after `then`; and the same two scenes of a fresh document.
#[allow(clippy::type_complexity)]
fn edited(
    cx: &mut TestAppContext,
    page: impl Fn(&str) -> String,
    then: impl Fn(&gpui::Entity<View>, &mut VisualTestContext),
) -> (
    bool,
    usize,
    (Vec<String>, Vec<String>),
    (Vec<String>, Vec<String>),
) {
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    window(vcx, 300.);
    update(&view, vcx, &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit"));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let waits = waiting(&view, vcx);
    let now = quads(vcx);
    then(&view, vcx);
    let edited = (now, quads(vcx));
    let (fview, fresh) = open(cx, &page(" data-edit"));
    window(fresh, 300.);
    let first = quads(fresh);
    then(&fview, fresh);
    (kept, waits, edited, (first, quads(fresh)))
}

fn grown(_: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    window(cx, 4200.);
}

/// The edit keeps the frame and waits (`waits`), and every frame equals a
/// fresh document's, in a taller window and in a taller viewport.
fn check(cx: &mut TestAppContext, page: impl Fn(&str) -> String + Copy, waits: bool) {
    let thens: [fn(&gpui::Entity<View>, &mut VisualTestContext); 2] = [grown, tall];
    for then in thens {
        let (kept, waiting, edited, fresh) = edited(cx, page, then);
        assert!(
            !fresh.0.is_empty() && !fresh.1.is_empty(),
            "empty reference scene"
        );
        assert_eq!(waiting > 0, waits, "edits waiting: {waiting}");
        assert!(!waits || kept, "a waiting edit kept no frame");
        assert_eq!(
            edited.0, fresh.0,
            "the edited frame differs from a fresh one"
        );
        assert_eq!(
            edited.1, fresh.1,
            "on the screen it differs from a fresh one"
        );
    }
}

#[gpui::test]
fn a_band_on_a_justified_line_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| spans(150, e, false, ""), true);
}

#[gpui::test]
fn a_band_on_a_justified_line_on_the_screen_builds_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| spans(1, e, false, ""), false);
}

#[gpui::test]
fn a_band_on_a_nested_box_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| spans(150, e, true, ""), true);
}

#[gpui::test]
fn a_shadow_reaching_the_screen_from_a_band_below_is_not_left_waiting(cx: &mut TestAppContext) {
    let css = "#s20[data-edit]{box-shadow:0 -400px 0 red}";
    check(cx, |e| spans(20, e, false, css), false);
}

#[path = "offscreen_tests/columns.rs"]
mod columns;
#[path = "offscreen_tests/culled.rs"]
mod culled;
#[path = "offscreen_tests/decorated.rs"]
mod decorated;
#[path = "offscreen_tests/grouped.rs"]
mod grouped;

/// The path of the element with `id` in `nodes`.
fn path_of(nodes: &[Node], id: &str) -> Option<Vec<usize>> {
    nodes.iter().enumerate().find_map(|(i, n)| {
        let Node::Element(e) = n else { return None };
        if e.attr("id") == Some(id) {
            return Some(vec![i]);
        }
        let mut p = path_of(&e.children, id)?;
        p.insert(0, i);
        Some(p)
    })
}

fn rects(
    view: &gpui::Entity<View>,
    cx: &mut VisualTestContext,
    id: &str,
) -> Option<Vec<gpui::Bounds<gpui::Pixels>>> {
    cx.update(|_, cx| {
        let doc = &view.read(cx).doc;
        doc.inline_rects(&path_of(doc.nodes(), id)?)
    })
}

#[gpui::test]
fn the_fragments_of_a_wrapped_box_are_its_lines_and_hold_through_a_waiting_edit(
    cx: &mut TestAppContext,
) {
    let page = |e: &str| spans(150, e, true, "");
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    window(vcx, 300.);
    update(&view, vcx, &page(""));
    let outer = rects(&view, vcx, "o150").expect("the fragments of a painted paragraph");
    assert!(
        outer.len() >= 2,
        "a box over two lines has two fragments: {outer:?}"
    );
    for pair in outer.windows(2) {
        assert_eq!(
            pair[1].origin.y - pair[0].origin.y,
            px(20.),
            "one line apart"
        );
    }
    // A nested box shares the fragments of its band owner, which hold it.
    assert_eq!(rects(&view, vcx, "s150"), Some(outer.clone()));
    // A waiting edit keeps the frame; the synchronous query reads the same
    // fragments without a build (the JS contract: no layout flush of the
    // document for the geometry of one box).
    let before = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit"));
    assert!(waiting(&view, vcx) > 0);
    let after = frames(&view, vcx);
    assert_eq!(rects(&view, vcx, "o150"), Some(outer));
    assert_eq!(frames(&view, vcx), after, "the query built a frame");
    assert_eq!(after.0, before.0, "the waiting edit built a frame");
}

#[gpui::test]
fn justified_and_left_lines_have_the_same_fragments(cx: &mut TestAppContext) {
    let tops = |cx: &mut TestAppContext, css: &str| {
        let html = spans(150, "", false, css);
        let (view, vcx) = settled(cx, &html, &html);
        window(vcx, 300.);
        update(&view, vcx, &html);
        let r = rects(&view, vcx, "s150").expect("fragments");
        r.iter().map(|b| b.origin.y).collect::<Vec<_>>()
    };
    let justified = tops(cx, "");
    assert_eq!(justified, tops(cx, "p{text-align:left}"));
}

#[gpui::test]
fn text_moved_above_a_box_is_never_read_from_old_fragments(cx: &mut TestAppContext) {
    // Span 150 far below; the spans above it go, so it moves onto the
    // screen. Its edit then equals a fresh document's, with or without a
    // frame between the two edits.
    let short =
        |e: &str| spans(150, e, false, "").replacen(&format!("<span>{WORDS}</span>"), "", 149);
    for framed in [true, false] {
        let page = |e: &str| spans(150, e, false, "");
        let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
        window(vcx, 300.);
        update(&view, vcx, &page(""));
        match framed {
            true => update(&view, vcx, &short("")),
            false => {
                vcx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&short(""), "").unwrap()));
            }
        }
        update(&view, vcx, &short(" data-edit"));
        assert_eq!(waiting(&view, vcx), 0, "an edit on the screen waits");
        let edited = quads(vcx);
        let (_, fresh) = open(cx, &short(" data-edit"));
        window(fresh, 300.);
        assert_eq!(edited, quads(fresh), "framed between: {framed}");
    }
}
