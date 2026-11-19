//! A new background colour of an inline box (`revision::layer_write`): the
//! box's band, its pieces over all its lines, is its runs in the document's
//! scene layer with the fill bound as its background
//! (`gpui::TextRun::background_owner`), so the revision keeps the frame and
//! writes the colour into every piece. A box with a border binds no piece
//! and builds the frame. Either frame equals a fresh document's.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// `body` (a block of text with an inline box `#s` over several lines) under
/// the rules `css`; `class` goes on `#s`.
fn page(css: &str, body: &str, class: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         p{{width:120px;font:16px/20px sans-serif;margin:0}} #s{{background:#ee0}} {css}</style>\
         </head><body>{}</body></html>",
        body.replace("CLASS", class)
    )
}

const WRAPPED: &str =
    "<p>one <span id=s class='CLASS'>two three four five six seven</span> end</p>";

/// Whether `class` on a settled document kept its frame, its frame and a
/// fresh document's.
fn edit(
    cx: &mut TestAppContext,
    css: &str,
    body: &str,
    class: &str,
) -> (bool, Vec<String>, Vec<String>) {
    let at = |class: &str| page(css, body, class);
    let (view, vcx) = settled(cx, &at(""), &at("x"));
    update(&view, vcx, &at(""));
    update(&view, vcx, &at(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &at(class));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &at(class));
    frame(fresh);
    (kept, edited, quads(fresh))
}

#[gpui::test]
fn a_wrapped_inline_box_takes_its_new_background_in_every_piece(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#0af}", WRAPPED, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_nested_box_keeps_its_own_band(cx: &mut TestAppContext) {
    let body = "<p>one <span id=s class='CLASS'>two <b style='background:#f0a'>three four</b> five six</span></p>";
    let (kept, edited, fresh) = edit(cx, "#s.b{background:#0af}", body, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_justified_box_takes_its_new_background_in_its_gaps(cx: &mut TestAppContext) {
    let css = "p{text-align:justify} #s.b{background:#0af}";
    let (kept, edited, fresh) = edit(cx, css, WRAPPED, "b");
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
}

#[gpui::test]
fn a_bordered_box_builds_its_frame_as_fresh(cx: &mut TestAppContext) {
    let css = "#s{border-left:2px solid #00f} #s.b{background:#0af}";
    let (kept, edited, fresh) = edit(cx, css, WRAPPED, "b");
    assert!(!kept, "a bordered band binds no piece: the frame is built");
    assert_eq!(edited, fresh, "the built frame differs from a fresh one");
}
