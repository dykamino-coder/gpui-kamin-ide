//! Content records across a change of the window width (`layout::reuse::content`):
//! the content of a cell whose column keeps its width replays, and a content
//! whose result follows the window (viewport and container lengths, a column
//! sized from the window, a media query on the width) is built. Every frame
//! equals a fresh document's at the same width.

use super::layered_replay_tests::scene;
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A 3×3 table in a wrapper block (not a root-flow block, whose records
/// replay it whole): `table` styles the table, `span` each cell's span.
fn cells(table: &str, span: &str, css: &str) -> String {
    let cells: String = (0..9)
        .map(|i| {
            let row = if i % 3 == 0 { "<tr>" } else { "" };
            format!("{row}<td><span style='{span}'>cell {i}</span></td>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} td{{padding:2px;border:1px solid #333}} \
         {css}</style></head><body><div><table style='{table}'>{cells}</table></div></body></html>"
    )
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

/// The scene of a fresh document `html` at window width `width`.
fn fresh(cx: &mut TestAppContext, html: &str, width: f32) -> Vec<String> {
    let (view, vcx) = open(cx, html);
    cx_width(&view, vcx, width);
    frame(vcx);
    scene(vcx)
}

fn cx_width(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
}

/// Open `html` at 400, resize to 640 (recording at the new width) and back to
/// 400: the contents replayed by the last frame, which must equal a fresh
/// document's at 400; and the frame at 640 must equal a fresh one at 640.
fn resized(cx: &mut TestAppContext, html: &str) -> u64 {
    let (view, vcx) = open(cx, html);
    frame(vcx);
    resize(&view, vcx, 640.);
    let wide = scene(vcx);
    let start = replayed(&view, vcx);
    resize(&view, vcx, 400.);
    let replays = replayed(&view, vcx) - start;
    let narrow = scene(vcx);
    assert_eq!(
        wide,
        fresh(cx, html, 640.),
        "the frame at 640 differs from a fresh one"
    );
    assert_eq!(
        narrow,
        fresh(cx, html, 400.),
        "the frame at 400 differs from a fresh one"
    );
    replays
}

const FIXED: &str = "table-layout:fixed;width:300px";

#[gpui::test]
fn cells_of_a_fixed_column_replay_their_content_across_a_width_change(cx: &mut TestAppContext) {
    let replays = resized(cx, &cells(FIXED, "", ""));
    assert!(replays >= 9, "clean cell contents replayed: {replays}");
}

#[gpui::test]
fn contents_with_viewport_lengths_are_built_after_a_width_change(cx: &mut TestAppContext) {
    for span in [
        "display:inline-block;width:10vw",
        "display:inline-block;padding-left:calc(1px + 1vw)",
        "display:inline-block;margin-left:1vmin",
        "display:inline-block;height:2vh",
        "font-size:calc(10px + 1vw)",
    ] {
        let replays = resized(cx, &cells(FIXED, span, ""));
        assert_eq!(replays, 0, "{span}");
    }
}

#[gpui::test]
fn contents_with_container_lengths_of_an_outer_container_are_built(cx: &mut TestAppContext) {
    let css = "div{container-type:inline-size}";
    let span = "display:inline-block;width:10cqw";
    assert_eq!(resized(cx, &cells(FIXED, span, css)), 0);
}

#[gpui::test]
fn cells_of_a_table_sized_by_the_window_are_built(cx: &mut TestAppContext) {
    // The column widths follow the window: so do the percentages inside.
    for span in ["", "display:inline-block;width:50%"] {
        let table = "table-layout:fixed;width:50%";
        assert_eq!(resized(cx, &cells(table, span, "")), 0, "{span}");
    }
}

#[gpui::test]
fn percentages_of_a_fixed_cell_replay_with_their_content(cx: &mut TestAppContext) {
    let replays = resized(cx, &cells(FIXED, "display:inline-block;width:50%", ""));
    assert!(replays >= 9, "percentages of a fixed column: {replays}");
}

#[gpui::test]
fn a_media_query_on_the_width_rebuilds_the_contents_it_restyles(cx: &mut TestAppContext) {
    let css = "@media (min-width:600px){td span{display:inline-block;padding-left:7px}}";
    assert_eq!(resized(cx, &cells(FIXED, "", css)), 0);
}
