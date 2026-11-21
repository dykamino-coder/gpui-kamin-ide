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
    page_with(
        attr,
        "repeating-conic-gradient(from 13deg,#e457,#37e8 10deg,#fc98 30deg);box-shadow:0 0 9px 4px #4067",
    )
}

/// `page` with the layers' background `fill` (and any declarations after it).
fn page_with(attr: &str, fill: &str) -> String {
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
         background:{fill};filter:blur(.6px);transform:rotate(7deg);\
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

/// The rendered frame's group labels and each group's bounds and quads.
fn groups(cx: &mut gpui::VisualTestContext) -> String {
    cx.update(|window, _| format!("{:?}", window.painted_groups()))
}

/// Count `name` of `crate::paint_path::NAMES` now.
fn path_count(name: &str) -> u64 {
    let at = crate::paint_path::NAMES.iter().position(|n| *n == name);
    crate::paint_path::read()[at.expect("a path counter")]
}

#[gpui::test]
fn a_colour_edit_inside_effect_groups_stays_in_the_kept_layer(cx: &mut TestAppContext) {
    // The layers' own run is painted inside their filter/blend groups: the
    // document's layer keeps the groups and the run is replaced in its group.
    let (none, set) = (page_with("", "#e45"), page_with(" data-edit=\"1\"", "#e45"));
    let (view, vcx) = settled(cx, &none, &set);
    update(&view, vcx, &none);
    let mut edited = Vec::new();
    for (html, name) in [(&set, "set"), (&none, "unset")] {
        let (restyled, no_layer, by_layer) = (
            path_count("plan_restyled"),
            path_count("refused_no_layer"),
            path_count("refused_by_layer"),
        );
        update(&view, vcx, html);
        assert!(
            path_count("plan_restyled") > restyled,
            "{name}: not restyled in place"
        );
        assert_eq!(
            path_count("refused_no_layer"),
            no_layer,
            "{name}: the layer was not kept"
        );
        assert_eq!(
            path_count("refused_by_layer"),
            by_layer,
            "{name}: the layer refused the run"
        );
        edited.push((html.clone(), name, quads(vcx), groups(vcx)));
    }
    for (html, name, edited, edited_groups) in edited {
        let (_, fresh) = open(cx, &html);
        frame(fresh);
        assert_eq!(edited, quads(fresh), "{name}: the edited frame differs");
        assert_eq!(
            edited_groups,
            groups(fresh),
            "{name}: the edited groups differ"
        );
    }
}
