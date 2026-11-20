//! A new fill colour of a box whose decorations paint as elements of their
//! own (border sides of another colour, a shadow): the layer takes it as a
//! value without a build of the frame when the decorations stay as painted,
//! and the frame equals a fresh document's; a change the decorations read
//! (a border colour, a fill under a shadow, a fill that appears) builds it.

use super::box_repaint_tests::{first_difference, frames, fresh, quads};
use super::reuse_tests::{settled, update};
use gpui::TestAppContext;

fn page(css: &str, edit: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         .b{{height:20px;border:2px solid #000;border-left-color:#00f;background:#456}}\
         [data-edit]{{background:#eee}} {css}</style></head><body>\
         <div class=b>one</div><div class=b{edit}><p>two</p></div><div class=b>three</div>\
         </body></html>"
    )
}

/// For each edit of the second box from a settled frame of `page(css, "")`:
/// whether the frame replayed without a build, and the first difference of
/// its scene from a fresh document's.
fn edits(cx: &mut TestAppContext, css: &str, steps: &[&str]) -> Vec<(bool, Option<String>)> {
    let (view, vcx) = settled(cx, &page(css, ""), &page(css, " data-x"));
    update(&view, vcx, &page(css, ""));
    update(&view, vcx, &page(css, ""));
    let mut out = Vec::new();
    for step in steps {
        let before = frames(&view, vcx);
        update(&view, vcx, &page(css, step));
        let replayed = frames(&view, vcx) == (before.0, before.1 + 1);
        out.push((replayed, quads(vcx)));
    }
    steps
        .iter()
        .zip(out)
        .map(|(step, (replayed, scene))| {
            let reference = fresh(cx, &page(css, step));
            let diff = first_difference(&scene, &reference).map(|d| format!("{d:?}"));
            (replayed, diff)
        })
        .collect()
}

#[gpui::test]
fn a_decorated_box_takes_a_new_fill_colour_without_a_build(cx: &mut TestAppContext) {
    let steps = [" data-edit", "", " data-edit"];
    for (i, (replayed, diff)) in edits(cx, "", &steps).into_iter().enumerate() {
        assert!(replayed, "edit {:?} built the frame", steps[i]);
        assert_eq!(diff, None, "edit {:?} differs from a fresh frame", steps[i]);
    }
}

#[gpui::test]
fn a_change_the_decorations_read_builds_the_frame(cx: &mut TestAppContext) {
    let cases = [
        // The border side of another colour is a decoration.
        "[data-edit]{border-left-color:#0f0}",
        // An outer shadow reads whether the fill is opaque.
        ".b{box-shadow:0 0 0 3px #444}",
    ];
    for css in cases {
        for (replayed, diff) in edits(cx, css, &[" data-edit", ""]) {
            assert_eq!(diff, None, "{css}: the frame differs from a fresh one");
            assert!(!replayed, "{css}: the decorations were left as painted");
        }
    }
}

#[gpui::test]
fn a_fill_that_appears_and_goes_on_a_decorated_box_shows_as_fresh(cx: &mut TestAppContext) {
    // No fill was painted to write into: whichever path takes it, the frame
    // equals a fresh document's.
    for (_, diff) in edits(cx, ".b{background:none}", &[" data-edit", "", " data-edit"]) {
        assert_eq!(diff, None, "the frame differs from a fresh one");
    }
}
