//! A page that is one fixed-width table across window width changes: the
//! record of the table, the only block of the root flow, replays it whole, so
//! no element of it is built and no cell replays its content. Cell contents
//! replayed by an earlier frame copy the selection listeners of their text,
//! which their record allowed (`gpui::RetainedPaint`): they do not keep the
//! table around them from its record. A table whose result follows the window
//! (a percentage width, a viewport length, a container or media query on the
//! width) is built again.

use super::super::layered_replay_tests::scene;
use super::super::tests::{View, frame, open};
use super::{replayed, resize};
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};

/// One table of 30 rows of 8 cells, an inline-block span in each, under `css`.
fn page(css: &str) -> String {
    let rows: String = (0..30)
        .map(|r| {
            let cells: String = (0..8)
                .map(|c| format!("<td><span>{r}-{c} a b c</span></td>"))
                .collect();
            format!("<tr>{cells}</tr>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} \
         table{{width:780px;border-collapse:collapse}} td{{border:1px solid #579;padding:1px}} \
         td:nth-child(3n){{width:5%}} span{{display:inline-block;max-width:80px}} {css}\
         </style></head><body><table>{rows}</table></body></html>"
    )
}

fn contents(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

/// Open `html` and resize it to 800, 801, 800 and 801: the root-flow blocks
/// and the cell contents replayed by the last two frames, and the last scene.
fn resized(cx: &mut TestAppContext, html: &str) -> ((u64, u64), Vec<String>) {
    let (view, cx) = open(cx, html);
    // The pointer of a test window rests at its origin: on a paragraph of the
    // table, whose hover listeners would keep it from a record.
    cx.simulate_mouse_move(point(px(795.), px(590.)), None, Modifiers::default());
    resize(&view, cx, 800.);
    resize(&view, cx, 801.);
    let (blocks, cells) = (replayed(&view, cx), contents(&view, cx));
    resize(&view, cx, 800.);
    resize(&view, cx, 801.);
    let counts = (replayed(&view, cx) - blocks, contents(&view, cx) - cells);
    (counts, scene(cx))
}

/// The scene of a fresh document `html` at window width 801.
fn fresh(cx: &mut TestAppContext, html: &str) -> Vec<String> {
    let (view, cx) = open(cx, html);
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = 801.));
    frame(cx);
    scene(cx)
}

#[gpui::test]
fn a_fixed_width_table_alone_in_the_flow_is_replayed_whole_across_width_changes(
    cx: &mut TestAppContext,
) {
    let html = page("");
    let ((blocks, cells), last) = resized(cx, &html);
    assert_eq!(blocks, 2, "the table was built at a width change");
    assert_eq!(cells, 0, "cells replayed their contents in a built table");
    assert_eq!(
        last,
        fresh(cx, &html),
        "the frame at 801 differs from a fresh one"
    );
}

#[gpui::test]
fn a_table_that_follows_the_window_is_built_at_width_changes(cx: &mut TestAppContext) {
    for css in [
        "table{width:50%}",
        "span{max-width:10vw}",
        "@media (min-width:801px){td{padding:3px}}",
    ] {
        let html = page(css);
        let ((blocks, _), last) = resized(cx, &html);
        assert_eq!(blocks, 0, "the table was replayed: {css}");
        assert_eq!(last, fresh(cx, &html), "the frame at 801 differs: {css}");
    }
    // A container whose size no value reads (no `@container` rule is parsed,
    // no `cq*` length): whatever replays, the frame is the fresh one.
    let html =
        page("body{container-type:inline-size} @container (min-width:801px){td{padding:3px}}");
    let (_, last) = resized(cx, &html);
    assert_eq!(
        last,
        fresh(cx, &html),
        "the frame at 801 differs under a container"
    );
}
