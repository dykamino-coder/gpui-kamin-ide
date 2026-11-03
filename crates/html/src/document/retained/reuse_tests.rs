//! Пропуск блоков корневого потока вне экрана после правки (`layout::reuse`).
//! Пропускаются блоки, отстоящие от экрана дальше его размера: документы
//! длиннее нескольких экранов. Первая сборка документа ничего не пишет:
//! записи делает вторая сборка с тем же ключом потока, пропускает третья.

use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn rows(edit: usize, text: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let body = if i == edit {
                text.to_string()
            } else {
                format!("row {i}")
            };
            format!("<div style='height:20px'>{body}</div>")
        })
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

fn reused(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.reused_blocks())
}

fn update(view: &gpui::Entity<View>, cx: &mut VisualTestContext, html: &str) {
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(html, "").unwrap()));
    frame(cx);
}

/// Open `first`, then rebuild with `second`: the second build records.
fn settled<'a>(
    cx: &'a mut TestAppContext,
    first: &str,
    second: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    let (view, cx) = open(cx, first);
    frame(cx);
    assert_eq!(
        reused(&view, cx),
        (0, 0, 0),
        "the first build of a document records nothing"
    );
    update(&view, cx, second);
    (view, cx)
}

#[gpui::test]
fn an_offscreen_edit_skips_the_unchanged_offscreen_blocks(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, skipped, rebuilt) = reused(&view, cx);
    assert!(
        recorded > 400,
        "settled blocks were not recorded: {recorded}"
    );
    assert_eq!((skipped, rebuilt), (0, 0));
    update(&view, cx, &rows(300, "row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(skipped > 300, "offscreen blocks were rebuilt: {skipped}");
    assert_eq!(rebuilt, 0, "no skipped block reached the screen");
}

#[gpui::test]
fn a_skipped_block_that_reaches_the_screen_rebuilds_the_frame(cx: &mut TestAppContext) {
    let tall = |end: &str| format!("{}{end}", "x ".repeat(6000));
    let page = |text: &str| rows(5, text).replace("<div style='height:20px'>x", "<div>x");
    let (view, cx) = settled(cx, &page(&tall("x")), &page(&tall("y")));
    update(&view, cx, &page("x"));
    frame(cx);
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "blocks pulled into view were left skipped");
}

#[gpui::test]
fn blocks_below_a_grown_block_are_skipped_at_their_new_place(cx: &mut TestAppContext) {
    let grown = format!("{}x", "x ".repeat(300));
    let page = |text: &str| rows(5, text).replace("<div style='height:20px'>x", "<div>x");
    let (view, cx) = settled(cx, &page("x"), &page("xy"));
    update(&view, cx, &page(&grown));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "shifted offscreen blocks were rebuilt: {skipped}"
    );
    // The grown block moves the visible blocks below it: their replay is
    // refused, and one more build of the same frame builds just them.
    assert_eq!(rebuilt, 1, "moved visible blocks were replayed");
    // An update adopts a new tree, and its frame counters start anew.
    let builds = cx.update(|_, cx| view.read(cx).doc.retained_frames().0);
    assert_eq!(builds, 2, "the refused frame was built more than twice");
}

#[gpui::test]
fn blocks_after_a_float_are_always_built(cx: &mut TestAppContext) {
    let float = "<div style='height:20px'><span style='float:left;height:9000px'>f</span></div>";
    let page =
        |text: &str| rows(300, text).replacen("<div style='height:20px'>row 2</div>", float, 1);
    let (view, cx) = settled(cx, &page("row 300"), &page("row edited"));
    update(&view, cx, &page("row 300"));
    let (_, skipped, _) = reused(&view, cx);
    // Only the two rows before the float are skipped (replayed on screen).
    assert!(
        skipped <= 2,
        "a float may reach into the blocks after it: {skipped}"
    );
}

#[gpui::test]
fn a_flow_resized_every_frame_records_nothing_and_drops_its_records(cx: &mut TestAppContext) {
    // Centred rows depend on the flow width (one-line rows at the start
    // edge would not: `one_line_text_blocks_are_reused_across_a_width_change`).
    let rows = |edit: usize, text: &str| {
        rows(edit, text).replace("height:20px'", "height:20px;text-align:center'")
    };
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, _, _) = reused(&view, cx);
    for width in [401., 400.] {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
        frame(cx);
    }
    let (after, skipped, rebuilt) = reused(&view, cx);
    assert_eq!(after, recorded, "records of a changing flow never pay off");
    // Back at the recorded width: records of another key were dropped (they
    // would hold the styles of their frame), so nothing is skipped.
    assert_eq!((skipped, rebuilt), (0, 0));
    update(&view, cx, &rows(300, "row 300"));
    let (again, skipped, _) = reused(&view, cx);
    assert!(again > recorded + 400 && skipped == 0, "{again} {skipped}");
    update(&view, cx, &rows(300, "row edited"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(skipped > 300 && rebuilt == 0, "{skipped} {rebuilt}");
}

/// Blocks near the screen are recorded with their frame output and replayed
/// in place by the next frame, the visible ones included.
#[gpui::test]
fn blocks_on_the_screen_are_replayed_from_the_previous_frame(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, _, _) = reused(&view, cx);
    assert!(recorded > 590, "near blocks were not recorded: {recorded}");
    update(&view, cx, &rows(300, "row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    let replayed = replayed(&view, cx);
    // 600 rows of 20px in a 1920x1080 test window: 54 rows are visible.
    assert!(replayed > 50, "visible blocks were rebuilt: {replayed}");
    assert!(skipped > 590 && rebuilt == 0, "{skipped} {rebuilt}");
}

#[gpui::test]
fn blocks_of_unwrapped_documents_are_skipped(cx: &mut TestAppContext) {
    for body in ["", " style='padding:3px'"] {
        unwrapped(cx, body);
    }
}

fn unwrapped(cx: &mut TestAppContext, body: &str) {
    let page = |edit: &str| -> String {
        let paragraphs: String = (0..600)
            .map(|i| {
                let word = if i == 300 { edit } else { "edit" };
                format!("<p>שלום עולם {word} {i} مرحبا <b>מודגש</b> text.</p>\n")
            })
            .collect();
        format!(
            "<!doctype html><html dir=rtl><head><style>p{{margin:2px 0}}</style></head><body{body}>\n{paragraphs}</body></html>"
        )
    };
    let (view, cx) = settled(cx, &page("edit"), &page("odit"));
    update(&view, cx, &page("edit"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "unwrapped root flow was rebuilt: {body} {skipped}"
    );
    assert_eq!(rebuilt, 0);
}

#[gpui::test]
fn right_to_left_blocks_are_skipped_too(cx: &mut TestAppContext) {
    let page = |text: &str| format!("<html dir=rtl>{}</html>", rows(300, text));
    let (view, cx) = settled(cx, &page("row 300"), &page("row edited"));
    update(&view, cx, &page("row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "right-to-left blocks were rebuilt: {skipped}"
    );
    assert_eq!(rebuilt, 0);
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
