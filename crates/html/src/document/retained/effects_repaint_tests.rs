//! A `data-*` edit that swaps the background of a transformed, filtered and
//! blended box (effects-480 D: a conic gradient becomes `#eee`): a paint-only
//! restyle (`revision::repaint`), whose frame equals a fresh document's both
//! ways.

use super::box_repaint_tests::quads;
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// Five overlapping layers; the middle one carries `attr`.
fn page(attr: &str) -> String {
    let mut body = String::new();
    for i in 0..5 {
        let at = if i == 2 { attr } else { "" };
        body.push_str(&format!(
            "<div class=\"layer\"{at} style=\"left:{}px;top:{}px\"></div>",
            i * 31,
            i * 47
        ));
    }
    format!(
        "<!doctype html><html><head><style>html{{background:white}}body{{margin:0}}\
         .layer{{position:absolute;width:180px;height:140px;border-radius:25%;\
         background:repeating-conic-gradient(from 13deg,#e457,#37e8 10deg,#fc98 30deg);\
         box-shadow:0 0 9px 4px #4067;filter:blur(.6px);transform:rotate(7deg);\
         mix-blend-mode:multiply}}[data-edit]{{background:#eee}}</style></head>\
         <body>{body}</body></html>"
    )
}

#[gpui::test]
fn a_background_edit_of_an_effect_layer_equals_a_fresh_frame(cx: &mut TestAppContext) {
    let (none, set) = (page(""), page(" data-edit=\"1\""));
    let (view, vcx) = settled(cx, &none, &set);
    update(&view, vcx, &none);
    let mut edited = Vec::new();
    for (html, name) in [(&set, "set"), (&none, "unset")] {
        let restyled = crate::paint_path::read();
        update(&view, vcx, html);
        let at = crate::paint_path::NAMES
            .iter()
            .position(|n| *n == "plan_restyled");
        let at = at.expect("the plan counter");
        assert!(
            crate::paint_path::read()[at] > restyled[at],
            "{name}: not restyled in place"
        );
        edited.push((html.clone(), name, quads(vcx)));
    }
    for (html, name, edited) in edited {
        let (_, fresh) = open(cx, &html);
        frame(fresh);
        assert_eq!(edited, quads(fresh), "{name}: the edited frame differs");
    }
}
