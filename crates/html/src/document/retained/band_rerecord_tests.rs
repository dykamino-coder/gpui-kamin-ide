//! A band that appears, goes or changes its paint on an inline box
//! (`revision::layer_band`): the pieces of the box over all its lines are
//! its runs in the document's scene layer, empty ones too, and are painted
//! again from the new band (`gpui::Window::rerecord_layer_band`), so the
//! revision keeps the frame. A change that moves pieces builds the frame.
//! Either frame equals a fresh document's.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// `body` under the rules `css`; `class` goes where `CLASS` stands.
fn page(css: &str, body: &str, class: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         p{{width:120px;font:16px/20px sans-serif;margin:0}} {css}</style>\
         </head><body>{}</body></html>",
        body.replace("CLASS", class)
    )
}

pub(super) const WRAPPED: &str =
    "<p>one <span id=s class='CLASS'>two three four five six seven</span> end</p>";

/// Whether `class` on a settled document kept its frame, its frame and a
/// fresh document's.
pub(super) fn edit(
    cx: &mut TestAppContext,
    css: &str,
    body: &str,
    class: &str,
) -> (bool, Vec<String>, Vec<String>) {
    let (kept, edited, fresh, _, _) = edit_refused(cx, css, body, class);
    (kept, edited, fresh)
}

/// `edit`, the paint path's refusals of the edit (process-wide: tests in
/// parallel add theirs) and the boxes the document queued to record again.
fn edit_refused(
    cx: &mut TestAppContext,
    css: &str,
    body: &str,
    class: &str,
) -> (bool, Vec<String>, Vec<String>, String, usize) {
    let at = |class: &str| page(css, body, class);
    let (view, vcx) = settled(cx, &at(""), &at("x"));
    update(&view, vcx, &at(""));
    update(&view, vcx, &at(""));
    let before = frames(&view, vcx);
    let counts = crate::paint_path::read();
    vcx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&at(class), "").unwrap()));
    let queued = vcx.update(|_, cx| {
        let reuse = &view.read(cx).doc.reuse;
        let queued = reuse.take_repaints();
        let n = queued.len();
        reuse.queue_repaints(queued);
        n
    });
    frame(vcx);
    let refused = delta(&counts);
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &at(class));
    frame(fresh);
    (kept, edited, quads(fresh), refused, queued)
}

#[gpui::test]
fn a_band_appears_in_every_piece_of_a_wrapped_box(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#0af}", WRAPPED, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_band_goes_from_every_piece(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(
        cx,
        "#s{background:#0af} #s.b{background:none}",
        WRAPPED,
        "b",
    );
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

/// The paint path's refusals since `counts`.
fn delta(counts: &[u64]) -> String {
    let now = crate::paint_path::read();
    let named = crate::paint_path::NAMES.iter().zip(now.iter().zip(counts));
    named
        .filter(|(n, (a, b))| {
            a > b && (n.starts_with("refused") || n.starts_with("not_own") || **n == "records")
        })
        .map(|(n, (a, b))| format!("{n}={} ", a - b))
        .collect()
}

/// A box with edges (padding, border) puts edge spacers on its lines, and a
/// line with edge spacers is painted word by word, as a justified one is
/// (`Paragraph::paint_justified_line`): its pieces are cut by that painter,
/// the layer holds none of them (`LayerHandle::holds_band`) and the frame
/// is built, equal to a fresh one.
#[gpui::test]
fn a_bordered_band_takes_a_background(cx: &mut TestAppContext) {
    let css = "#s{border:2px solid #00f;padding:1px} #s.b{background:#eee}";
    let (kept, edited, fresh, refused, _) = edit_refused(cx, css, WRAPPED, "b");
    assert!(
        !kept,
        "the pieces of a box with edges are cut: the frame is built"
    );
    assert!(
        refused.contains("refused_by_layer"),
        "the layer refuses, not the plan: {refused}"
    );
    assert_eq!(
        edited, fresh,
        "the frame differs from a fresh one: {refused}"
    );
}

#[gpui::test]
fn a_box_beside_bands_takes_its_own_between_them(cx: &mut TestAppContext) {
    let body = "<p><b style='background:#f0a'>one</b> <span id=s class='CLASS'>two three</span> \
                <i style='background:#0f0'>four</i></p>";
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#0af}", body, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn an_empty_box_has_nothing_to_paint(cx: &mut TestAppContext) {
    let body = "<div><div><i id=s class='CLASS'></i></div></div>";
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#eee}", body, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_band_on_a_nested_box_builds_its_frame(cx: &mut TestAppContext) {
    let body = "<p>one <span>two <span id=s class='CLASS'>three four</span> five</span></p>";
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#0af}", body, "b");
    assert!(
        !kept,
        "the descendant's runs pass between bands: the frame is built"
    );
    assert_eq!(edited, fresh, "the built frame differs from a fresh one");
}

#[gpui::test]
fn a_padded_band_that_appears_builds_its_frame(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(cx, "#s{padding:4px} #s.b{background:#0af}", WRAPPED, "b");
    assert!(
        !kept,
        "the band's padding moves the line's layer: the frame is built"
    );
    assert_eq!(edited, fresh, "the built frame differs from a fresh one");
}

#[gpui::test]
fn a_band_on_a_justified_box_builds_its_frame(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(
        cx,
        "p{text-align:justify} #s.b{background:#0af}",
        WRAPPED,
        "b",
    );
    assert!(
        !kept,
        "justified pieces are cut by another painter: the frame is built"
    );
    assert_eq!(edited, fresh, "the built frame differs from a fresh one");
}

/// A bordered `span` that is a flex item: the layer holds its own box but not
/// its band, whose pieces edge spacers cut. The revision refuses before any
/// box is recorded again (`revision::layer_write`), and the frame is built.
#[gpui::test]
fn a_cut_band_is_refused_before_any_record(cx: &mut TestAppContext) {
    let css = "div{display:flex} #s{border:1px solid #acd;padding:1px} #s.b{background:#eee}";
    let body = "<div><span id=s class='CLASS'>side word</span></div>";
    let (kept, edited, fresh, refused, queued) = edit_refused(cx, css, body, "b");
    assert!(
        !kept,
        "the cut band is not recorded again: the frame is built"
    );
    assert_eq!(queued, 0, "a box was queued to record again: {refused}");
    assert_eq!(
        edited, fresh,
        "the frame differs from a fresh one: {refused}"
    );
}
