//! Пропуск блоков корневого потока вне экрана после правки (`layout::reuse`).
//! Пропускаются блоки, отстоящие от экрана дальше его размера: документы
//! длиннее нескольких экранов.

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

#[gpui::test]
fn an_offscreen_edit_skips_the_unchanged_offscreen_blocks(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &rows(300, "row 300"));
    frame(cx);
    let (recorded, skipped, rebuilt) = reused(&view, cx);
    assert!(
        recorded > 500,
        "settled blocks were not recorded: {recorded}"
    );
    assert_eq!((skipped, rebuilt), (0, 0));
    update(&view, cx, &rows(300, "row edited"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(skipped > 300, "offscreen blocks were rebuilt: {skipped}");
    assert_eq!(rebuilt, 0, "no skipped block reached the screen");
}

#[gpui::test]
fn a_skipped_block_that_reaches_the_screen_rebuilds_the_frame(cx: &mut TestAppContext) {
    let tall = format!("{}x", "x ".repeat(6000));
    let (view, cx) = open(
        cx,
        &rows(5, &tall).replace("<div style='height:20px'>x", "<div>x"),
    );
    frame(cx);
    update(
        &view,
        cx,
        &rows(5, "x").replace("<div style='height:20px'>x", "<div>x"),
    );
    frame(cx);
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "blocks pulled into view were left skipped");
}

#[gpui::test]
fn blocks_below_a_grown_block_are_skipped_at_their_new_place(cx: &mut TestAppContext) {
    let grown = format!("{}x", "x ".repeat(300));
    let (view, cx) = open(
        cx,
        &rows(5, "x").replace("<div style='height:20px'>x", "<div>x"),
    );
    frame(cx);
    update(
        &view,
        cx,
        &rows(5, &grown).replace("<div style='height:20px'>x", "<div>x"),
    );
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "shifted offscreen blocks were rebuilt: {skipped}"
    );
    assert_eq!(rebuilt, 0);
}

#[gpui::test]
fn blocks_after_a_float_are_always_built(cx: &mut TestAppContext) {
    let float = "<div style='height:20px'><span style='float:left;height:9000px'>f</span></div>";
    let page =
        |text: &str| rows(300, text).replacen("<div style='height:20px'>row 2</div>", float, 1);
    let (view, cx) = open(cx, &page("row 300"));
    frame(cx);
    update(&view, cx, &page("row edited"));
    let (_, skipped, _) = reused(&view, cx);
    assert_eq!(skipped, 0, "a float may reach into the blocks after it");
}

#[gpui::test]
fn a_flow_resized_every_frame_records_nothing_and_keeps_its_records(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &rows(300, "row 300"));
    frame(cx);
    let (recorded, _, _) = reused(&view, cx);
    for width in [401., 400., 401.] {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
        frame(cx);
    }
    let (after, skipped, rebuilt) = reused(&view, cx);
    assert_eq!(after, recorded, "records of a changing flow never pay off");
    // Back at the recorded width the kept records still fit.
    assert!(skipped > 300 && rebuilt == 0, "{skipped} {rebuilt}");
}
