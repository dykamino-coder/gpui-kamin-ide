//! A band appearing on a culled inline box far below the screen.
use super::*;

/// Rows of plain text, a culled inline box (no band before the edit) in row
/// `at`: no band owner holds its text, its container's lines bound it.
fn culled(at: usize, edit: &str) -> String {
    let body: String = (0..200)
        .map(|i| match i == at {
            true => format!("<div>row {i} <span id=s{i}{edit}>deep</span> end</div>"),
            false => format!("<div>row {i}</div>"),
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         div{{font:16px/20px sans-serif}} [data-edit]{{background:#eee}}</style></head>\
         <body>{body}</body></html>"
    )
}

#[gpui::test]
fn a_band_appearing_on_a_culled_box_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| culled(150, e), true);
}
