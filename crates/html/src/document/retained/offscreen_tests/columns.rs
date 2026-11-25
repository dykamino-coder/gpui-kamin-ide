//! A band on a nested inline box of a list item in a multi-column list far
//! below the screen (a reference list): the item's paragraph is laid out in
//! a column piece the frame never painted.
use super::*;

/// An ordered list of 300 items in columns of 12em; `edit` goes on the inner
/// span of item `at`, nested in a `cite` in a span.
fn references(at: usize, edit: &str) -> String {
    let items: String = (0..300)
        .map(|i| {
            let mark = if i == at { edit } else { "" };
            format!(
                "<li><span class=r><cite>ref {i} <span id=s{i}{mark}>words</span> \
                 more</cite></span> tail {i}</li>"
            )
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         ol{{column-width:12em;font:16px/20px sans-serif;margin:0}}\
         [data-edit]{{background:#eee}}</style></head><body><ol>{items}</ol></body></html>"
    )
}

#[gpui::test]
fn a_band_on_a_nested_box_of_a_column_item_far_below_waits_and_shows_as_fresh(
    cx: &mut TestAppContext,
) {
    check(cx, |e| references(280, e), true);
}
