//! A block-level image wider than its container: `max-width: 100%` clamps its
//! used width before its height follows the ratio (CSS 2.1 §10.4 with §10.3.4
//! and §10.6.2), so it neither paints wider than the column nor widens the
//! scrollable overflow that decides the root's bars.

use super::tests::{frame, open_document};
use crate::Document;
use crate::resources::images::tests_support::{TempDir, png_sized};
use gpui::TestAppContext;

/// The border boxes (width, height) of the bordered boxes the page paints.
fn image_box(cx: &mut TestAppContext, column: &str) -> Vec<(f32, f32)> {
    let mut dir = TempDir::new("replaced-max-width");
    dir.write("big.png", &png_sized(900, 753, [9, 9, 9, 255]));
    let page = dir.path().join("page.html");
    let html = format!(
        "<style>*{{box-sizing:border-box}}body{{margin:0}}.c{{width:513px}}{column}\
         img{{height:auto;max-width:100%}}\
         section img{{display:block;margin:32px auto;border:1px solid #000}}</style>\
         <div class=c><section><p><img src='big.png' width=900 height=753></p></section></div>"
    );
    let doc = Document::builder(&html, "").path(&page).build().unwrap();
    let (_one, _sub) = crate::why::test_reader();
    let (_view, cx) = open_document(cx, doc);
    for _ in 0..3 {
        frame(cx);
    }
    // The picture fills its box: the root gets no horizontal bar from it.
    let journal = crate::why::take().journal;
    let root_bars = journal
        .causes
        .iter()
        .find(|c| c.code == "scroll_extent.layout_bars" && c.detail.starts_with("box 0 "));
    assert!(
        root_bars.is_none(),
        "{column:?}: the root's bars changed: {root_bars:?}"
    );
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let quads = window.painted_quads();
        let bordered = quads.iter().filter(|q| q.border_widths.left.0 > 0.0);
        bordered
            .map(|q| {
                (
                    q.bounds.size.width.0 / scale,
                    q.bounds.size.height.0 / scale,
                )
            })
            .collect()
    })
}

#[gpui::test]
fn a_block_image_wider_than_its_column_is_clamped_by_max_width(cx: &mut TestAppContext) {
    for column in [
        "",
        ".c{display:flex;flex-direction:column}",
        ".c{display:flex;flex-direction:column}section{display:flex;flex-direction:column}",
    ] {
        check(image_box(cx, column), column);
    }
}

fn check(bordered: Vec<(f32, f32)>, column: &str) {
    let widest = bordered.iter().map(|b| b.0).fold(0.0_f32, f32::max);
    assert!(
        (widest - 513.0).abs() < 1.0,
        "{column:?}: the image's border box is not the column's width: {bordered:?}"
    );
    // The content height follows the ratio of the clamped content width.
    let tall = bordered.iter().map(|b| b.1).fold(0.0_f32, f32::max);
    let expected = 511.0 * 753.0 / 900.0 + 2.0;
    assert!(
        (tall - expected).abs() < 1.0,
        "{column:?}: height {tall}: {bordered:?}"
    );
}
