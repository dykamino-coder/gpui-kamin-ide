//! Повтор блоков корневого потока после смены его ширины (`layout::reuse::width`):
//! блок, чья раскладка от ширины не зависит, переживает её.

use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn reused(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.reused_blocks())
}

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_blocks())
}

/// Open `html` at width 400 and resize it to 401 and back: the second build
/// records what does not depend on the width, the third reuses it.
fn resized(cx: &mut TestAppContext, html: &str) -> ((u64, u64, u64), u64) {
    let (view, cx) = open(cx, html);
    frame(cx);
    resize(&view, cx, 401.);
    resize(&view, cx, 400.);
    (reused(&view, cx), replayed(&view, cx))
}

fn fixed_rows(style: &str) -> String {
    let rows: String = (0..600)
        .map(|i| format!("<div style='width:300px;height:20px{style}'>row {i}</div>"))
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

#[gpui::test]
fn fixed_width_blocks_are_reused_across_a_width_change(cx: &mut TestAppContext) {
    let ((recorded, skipped, rebuilt), replayed) = resized(cx, &fixed_rows(""));
    assert!(
        recorded > 500,
        "fixed-width blocks were not recorded: {recorded}"
    );
    assert!(skipped > 500, "fixed-width blocks were rebuilt: {skipped}");
    assert!(
        replayed > 10,
        "visible blocks were not replayed: {replayed}"
    );
    assert_eq!(rebuilt, 0, "a replayed block failed its check");
}

#[gpui::test]
fn width_dependent_blocks_are_rebuilt_after_a_width_change(cx: &mut TestAppContext) {
    for style in [";margin-left:10%", ";padding:1vw"] {
        let ((recorded, skipped, _), _) = resized(cx, &fixed_rows(style));
        assert_eq!((recorded, skipped), (0, 0), "{style}");
    }
    let rtl = fixed_rows("").replace("<body ", "<body dir=rtl ");
    let ((recorded, skipped, _), _) = resized(cx, &rtl);
    assert_eq!(
        (recorded, skipped),
        (0, 0),
        "a right-to-left flow moves fixed-width blocks"
    );
}

fn table(style: &str) -> String {
    let cells: String = (0..12)
        .map(|i| format!("<td>unbreakable{i}content</td>"))
        .collect();
    let rows: String = (0..40).map(|_| format!("<tr>{cells}</tr>")).collect();
    // Whole-pixel rows: fractional line heights add up to an overflow a
    // thousandth of a pixel past the table box, and its leaf is not exact.
    format!(
        "<body style='margin:0'><h1>title</h1><table style='line-height:20px{style}'>{rows}</table></body>"
    )
}

/// A fixed-width table is the only and last block of the flow, on screen:
/// its frame output is replayed at the other width.
#[gpui::test]
fn a_fixed_width_table_is_replayed_across_a_width_change(cx: &mut TestAppContext) {
    let ((recorded, skipped, rebuilt), replayed) = resized(cx, &table(";width:300px"));
    // The title is a one-line block: recorded in both frames, stretched at
    // the other width and, being visible, built once more in the frame.
    assert_eq!((recorded, skipped), (3, 1), "the table was rebuilt");
    assert_eq!((replayed, rebuilt), (1, 1));
}

/// An auto-width table wider than the flow is held at its min-content width
/// (CSS 2.1 §17.5.2.2), and its wrapper box is exactly that wide
/// (css-tables-3): the same block at every narrower flow.
#[gpui::test]
fn an_overflowing_auto_width_table_is_replayed_across_a_width_change(cx: &mut TestAppContext) {
    let ((recorded, skipped, rebuilt), replayed) = resized(cx, &table(""));
    assert_eq!((recorded, skipped), (3, 1), "the table was rebuilt");
    assert_eq!((replayed, rebuilt), (1, 1));
}

/// A table narrower than the flow takes its max-content width; at the
/// width of its record only.
#[gpui::test]
fn an_auto_width_table_that_fits_the_flow_is_rebuilt_after_a_width_change(cx: &mut TestAppContext) {
    let narrow = table("").replace("unbreakable", "u").replace("content", "");
    let ((_, skipped, _), _) = resized(cx, &narrow);
    assert_eq!(skipped, 0, "a fitting table follows the flow width");
}

fn text_rows(style: &str) -> String {
    let rows: String = (0..600)
        .map(|i| format!("<div style='height:20px{style}'>row {i} alpha beta</div>"))
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

/// A one-line text block keeps its layout in a flow of another width: the
/// offscreen ones stay placeholders (stretched to the flow), the visible ones
/// are built again in the same frame (their background is wider).
#[gpui::test]
fn one_line_text_blocks_are_reused_across_a_width_change(cx: &mut TestAppContext) {
    let ((recorded, skipped, rebuilt), replayed) = resized(cx, &text_rows(""));
    assert!(
        recorded > 500,
        "one-line blocks were not recorded: {recorded}"
    );
    assert!(skipped > 500, "one-line blocks were rebuilt: {skipped}");
    assert_eq!(
        replayed, 0,
        "a stretched block repeated the paint of another width"
    );
    assert_eq!(
        rebuilt, 1,
        "the visible stretched blocks are built once more"
    );
}

/// Blocks whose text wraps or is centred depend on the flow width.
#[gpui::test]
fn wrapping_or_centred_text_blocks_are_rebuilt_after_a_width_change(cx: &mut TestAppContext) {
    let long = text_rows("").replace("alpha beta", &"alpha beta ".repeat(40));
    for page in [long, text_rows(";text-align:center")] {
        let ((_, skipped, _), _) = resized(cx, &page);
        assert_eq!(skipped, 0);
    }
}
