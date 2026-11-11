//! Повтор блоков корневого потока после смены его ширины (`layout::reuse::width`):
//! блок, чья раскладка от ширины не зависит, переживает её.

mod replay_records;

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
    // The title is a visible one-line block: its paint at the other width is
    // never replayable, so it is built, not refused, and the frame is built once.
    assert_eq!((recorded, skipped), (3, 1), "the table was rebuilt");
    assert_eq!((replayed, rebuilt), (1, 0));
}

/// An auto-width table wider than the flow is held at its min-content width
/// (CSS 2.1 §17.5.2.2) at the start edge of its wrapper, which paints nothing:
/// the same block at every narrower flow. Fixed margins keep it there, and so
/// do auto margins, which are zero for a box wider than its container
/// (§10.3.3 over-constrained; css-flexbox-1 §8.1 with negative free space).
#[gpui::test]
fn an_overflowing_auto_width_table_is_replayed_across_a_width_change(cx: &mut TestAppContext) {
    for style in ["", ";margin-left:7px", ";margin:0 auto"] {
        let ((recorded, skipped, rebuilt), replayed) = resized(cx, &table(style));
        assert_eq!(
            (recorded, skipped),
            (3, 1),
            "the table was rebuilt: {style}"
        );
        assert_eq!((replayed, rebuilt), (1, 0), "{style}");
    }
}

/// A table narrower than the flow takes its max-content width, so its place
/// (centred) or the flow width it was laid out in is the flow's: at the width
/// of its record only.
#[gpui::test]
fn an_auto_width_table_that_fits_the_flow_is_rebuilt_after_a_width_change(cx: &mut TestAppContext) {
    let rows: String = (0..40).map(|_| "<tr><td>a</td><td>b</td></tr>").collect();
    for style in ["", ";margin:0 auto"] {
        let page = format!(
            "<body style='margin:0'><h1>title</h1><table style='line-height:20px{style}'>{rows}</table></body>"
        );
        let ((_, skipped, _), _) = resized(cx, &page);
        assert_eq!(
            skipped, 0,
            "a fitting table follows the flow width: {style}"
        );
    }
}
fn text_rows(style: &str) -> String {
    let rows: String = (0..600)
        .map(|i| format!("<div style='height:20px{style}'>row {i} alpha beta</div>"))
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

/// A one-line text block keeps its layout in a flow of another width: the
/// offscreen ones stay placeholders (stretched to the flow), the visible ones
/// are built (their background is wider), so the frame is built once.
#[gpui::test]
fn one_line_text_blocks_are_reused_across_a_width_change(cx: &mut TestAppContext) {
    let ((recorded, skipped, rebuilt), replayed) = resized(cx, &text_rows(""));
    assert!(
        recorded > 500,
        "one-line blocks were not recorded: {recorded}"
    );
    // Blocks near the screen are built too: a stretched one cannot replay.
    assert!(skipped > 400, "one-line blocks were rebuilt: {skipped}");
    assert_eq!(
        replayed, 0,
        "a stretched block repeated the paint of another width"
    );
    assert_eq!(
        rebuilt, 0,
        "a visible stretched block was refused and the frame built again"
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

/// A block of text that wrapped while the flow width changed wraps in every
/// narrower flow, where its record would be refused: such builds do not wrap
/// it for recording. A wider flow, where its text fits on one line, records it.
#[gpui::test]
fn a_wrapped_text_block_is_not_tracked_in_a_narrower_flow(cx: &mut TestAppContext) {
    let page = text_rows("").replace("alpha beta", &"alpha beta ".repeat(6));
    let (view, cx) = open(cx, &page);
    frame(cx);
    let tracked = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks())
    };
    resize(&view, cx, 401.);
    let wrapped = tracked(&view, cx);
    assert!(
        wrapped > 500,
        "the wrapping blocks were not tracked: {wrapped}"
    );
    for width in [400., 401., 300.] {
        resize(&view, cx, width);
        assert_eq!(tracked(&view, cx), wrapped, "tracked again at {width}");
    }
    let before = reused(&view, cx).0;
    let far_before = cx.update(|_, cx| view.read(cx).doc.far_blocks());
    resize(&view, cx, 1200.);
    // A block far from the screen is laid out from its far record
    // (`layout::reuse::far`) without a build; every built one is tracked.
    let wide = tracked(&view, cx) - wrapped;
    let far = cx.update(|_, cx| view.read(cx).doc.far_blocks()) - far_before;
    assert!(
        wide + far > 500,
        "blocks in a wider flow were neither tracked nor far: {wide} {far}"
    );
    let (recorded, _, rebuilt) = reused(&view, cx);
    let recorded = recorded - before;
    // Every built one-line block records but at most one edge row of the flow.
    assert!(
        recorded <= wide && wide - recorded <= 1,
        "built one-line blocks were not recorded: {recorded} of {wide}"
    );
    assert_eq!(rebuilt, 0);
}

/// A build at a settled flow width learns which blocks wrapped there too, so
/// the first build at a narrower width already leaves them untracked.
#[gpui::test]
fn a_settled_build_learns_the_wrapped_blocks(cx: &mut TestAppContext) {
    let page = text_rows("").replace("alpha beta", &"alpha beta ".repeat(6));
    let (view, cx) = open(cx, &page);
    frame(cx);
    // An edit builds the frame again at the same width: a settled build.
    let edited = page.replace("row 599 ", "row edited ");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&edited, "").unwrap()));
    frame(cx);
    let tracked = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks())
    };
    let settled = tracked(&view, cx);
    assert!(
        settled > 500,
        "the settled build did not track the blocks: {settled}"
    );
    resize(&view, cx, 300.);
    let narrower = tracked(&view, cx) - settled;
    assert!(narrower < 50, "wrapped blocks tracked again: {narrower}");
}
