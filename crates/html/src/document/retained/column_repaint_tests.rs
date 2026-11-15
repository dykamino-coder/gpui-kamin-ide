//! Own paint of a box inside a child split across columns: the child's paint
//! is one record placed in each column (`layout::fragment::replay`), and a
//! paint-only restyle of a box in it reaches the document's scene layer
//! through the runs the record carried (`gpui` `scene::layer::placed`),
//! without a build of the frame; the frame equals a fresh document's.

use super::box_repaint_tests::{first_difference, frames, fresh, quads};
use super::reuse_tests::{settled, update};
use gpui::TestAppContext;

/// Six boxes in one block split over two balanced columns; `edit` goes on
/// the fifth, which the second column shows.
fn page(edit: &str) -> String {
    let boxes: String = (1..=6)
        .map(|i| {
            let e = if i == 5 { edit } else { "" };
            format!("<div class=b{e}>box {i}</div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         .m{{width:400px;columns:2;column-gap:20px;column-fill:balance}}\
         .b{{height:30px;margin:0 0 4px;background:#def}} [data-edit]{{background:#eee}}\
         [data-edit=\"2\"]{{background:#ddd}}</style></head><body>\
         <div class=m><div class=w>{boxes}</div></div></body></html>"
    )
}

#[gpui::test]
fn a_fill_in_a_split_child_is_recorded_in_every_column_without_a_build(cx: &mut TestAppContext) {
    let steps = [" data-edit", " data-edit=\"2\"", ""];
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let mut got = Vec::new();
    for step in steps {
        let before = frames(&view, vcx);
        update(&view, vcx, &page(step));
        let after = frames(&view, vcx);
        got.push((after == (before.0, before.1 + 1), quads(vcx)));
    }
    for (step, (replayed, scene)) in steps.iter().zip(got) {
        assert!(replayed, "edit {step:?} built the frame");
        let fresh = fresh(cx, &page(step));
        assert_eq!(
            first_difference(&scene, &fresh),
            None,
            "edit {step:?} differs from a fresh frame"
        );
    }
}
