//! A new background colour of an inline box (`revision::layer_write`): the
//! box's band, its pieces over all its lines, is its runs in the document's
//! scene layer with the fill bound as its background
//! (`gpui::TextRun::background_owner`), so the revision keeps the frame and
//! writes the colour into every piece. A box with a border binds no piece
//! and builds the frame. Either frame equals a fresh document's.

use super::band_rerecord_tests::WRAPPED;
use gpui::TestAppContext;

/// Whether `class` on `#s` (yellow before it) in `body` under `css` kept its
/// frame, its frame and a fresh document's (`band_rerecord_tests::edit`).
fn edit(
    cx: &mut TestAppContext,
    css: &str,
    body: &str,
    class: &str,
) -> (bool, Vec<String>, Vec<String>) {
    super::band_rerecord_tests::edit(cx, &format!("#s{{background:#ee0}} {css}"), body, class)
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
