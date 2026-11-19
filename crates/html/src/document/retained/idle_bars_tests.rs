//! Idle frames of a settled document build nothing: the scroll bars of each
//! scroll container stay with the box that owns its frame
//! (`interactive::scroll_extent::classic::model`), so no frame reports
//! `barsChanged` and defers another frame.

use super::tests::{frame, open};
use gpui::TestAppContext;

fn builds(view: &gpui::Entity<super::tests::View>, cx: &mut gpui::VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.retained_frames().0)
}

/// Builds over six idle frames after four settling frames.
fn idle_builds(cx: &mut TestAppContext, css: &str, body: &str) -> u64 {
    let html = format!(
        "<!doctype html><html><head><style>html{{overflow:auto}}body{{margin:0}}{css}</style>\
         </head><body><div style='height:3000px'>{body}</div></body></html>"
    );
    let (view, vcx) = open(cx, &html);
    for _ in 0..4 {
        frame(vcx);
    }
    let settled = builds(&view, vcx);
    for _ in 0..6 {
        frame(vcx);
    }
    builds(&view, vcx) - settled
}

#[gpui::test]
fn a_nested_scroll_container_is_idle_next_to_the_root(cx: &mut TestAppContext) {
    let body = "<div id=n style='overflow:auto;height:50px'><div style='height:400px'>\
                <span style='display:table-cell'>cell</span></div></div>";
    assert_eq!(
        idle_builds(cx, "", body),
        0,
        "idle frames rebuilt the document"
    );
}

/// A pseudo-element may be a scroll container (CSS Overflow 3 §3); it shares
/// node id 0 with the anonymous viewport and with anonymous boxes.
#[gpui::test]
fn a_scrolling_pseudo_element_is_idle(cx: &mut TestAppContext) {
    let css = "p::before{content:'a b c d e f g h';display:block;overflow:scroll;\
               height:20px;width:30px}";
    let body = "<p>x</p><span style='display:table-cell'>cell</span>";
    assert_eq!(
        idle_builds(cx, css, body),
        0,
        "idle frames rebuilt the document"
    );
}

#[gpui::test]
fn anonymous_boxes_under_the_root_are_idle(cx: &mut TestAppContext) {
    let css = "p::before{content:'x'}";
    let body = "<p>a</p><span style='display:table-cell'>cell</span>";
    assert_eq!(
        idle_builds(cx, css, body),
        0,
        "idle frames rebuilt the document"
    );
}
