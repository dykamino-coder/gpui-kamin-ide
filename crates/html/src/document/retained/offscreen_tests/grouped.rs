//! A band appearing on an inline box painted inside a composition group far
//! below the screen: a clipped visually hidden label, whose group shows no
//! pixel, and a translucent row.
use super::*;

/// Rows of plain text; row `at` holds a culled inline box (no band before
/// the edit) inside a box with `group`, which paints it in a group.
fn grouped(at: usize, edit: &str, group: &str) -> String {
    let body: String = (0..200)
        .map(|i| match i == at {
            true => format!(
                "<div>row {i} <label style='{group}'><span id=s{i}{edit}>deep</span> \
                 <input></label> end</div>"
            ),
            false => format!("<div>row {i}</div>"),
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         div{{font:16px/20px sans-serif;position:relative}} [data-edit]{{background:#eee}}\
         </style></head><body>{body}</body></html>"
    )
}

const HIDDEN: &str = "clip:rect(1px,1px,1px,1px);clip-path:inset(1px 1px 1px 1px);\
                      height:1px;overflow:hidden;position:absolute;width:1px";
const TRANSLUCENT: &str = "display:inline-block;opacity:0.5";

#[gpui::test]
fn a_band_in_a_clipped_hidden_box_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| grouped(150, e, HIDDEN), true);
}

#[gpui::test]
fn a_band_in_a_translucent_box_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| grouped(150, e, TRANSLUCENT), true);
}

#[gpui::test]
fn a_band_in_a_translucent_box_on_the_screen_builds_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| grouped(1, e, TRANSLUCENT), false);
}
