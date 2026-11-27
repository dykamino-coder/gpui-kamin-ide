//! The line split of a paragraph: prepaint reuses the layout's.

use super::tests::{frame, open};
use gpui::TestAppContext;

/// Prepaint takes the lines of the layout's final measure: a paragraph of a
/// fractional width is split once per frame, not again at its snapped width.
#[gpui::test]
fn prepaint_reuses_the_layouts_line_split(cx: &mut TestAppContext) {
    let words: String = (0..80).map(|i| format!("word{i} ")).collect();
    let html = format!("<p style='width:300.4px'>{words}</p>");
    let (view, cx) = open(cx, &html);
    frame(cx);
    let misses = cx.update(|_, cx| {
        view.read(cx)
            .doc
            .state
            .paragraph
            .paragraphs
            .split_misses
            .get()
    });
    assert_eq!(misses, 1, "prepaint split the paragraph again");
}
