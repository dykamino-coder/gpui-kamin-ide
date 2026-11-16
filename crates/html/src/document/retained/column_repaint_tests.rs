//! Own paint of a box inside a child split across columns: the child's paint
//! is one record placed in each column (`layout::fragment::replay`), and a
//! paint-only restyle of a box in it reaches the document's scene layer
//! through the runs the record carried (`gpui` `scene::layer::placed`),
//! without a build of the frame; the frame equals a fresh document's.

use super::box_repaint_tests::{first_difference, frames, fresh, quads};
use super::reuse_tests::{settled, update};
use super::tests::{View, frame};
use gpui::{TestAppContext, VisualTestContext};

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

fn resize(view: &gpui::Entity<View>, cx: &mut VisualTestContext, width: f32) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
    frame(cx);
}

#[gpui::test]
fn a_frame_built_after_a_fill_edit_records_the_new_fill_and_holds_the_box(cx: &mut TestAppContext) {
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let edited = page(" data-edit=\"2\"");
    update(&view, vcx, &edited);
    // A width change builds the frame: its layer records the edited fill.
    let width = vcx.update(|_, cx| view.read(cx).viewport.0);
    resize(&view, vcx, width + 40.);
    resize(&view, vcx, width);
    let built = quads(vcx);
    // The new layer holds the box in every column again: the next edit replays.
    let before = frames(&view, vcx);
    let next = page(" data-edit");
    update(&view, vcx, &next);
    let replayed = frames(&view, vcx) == (before.0, before.1 + 1);
    let after = quads(vcx);
    assert_eq!(
        first_difference(&built, &fresh(cx, &edited)),
        None,
        "the built frame differs from a fresh one"
    );
    assert!(replayed, "the edit after the build built the frame");
    assert_eq!(
        first_difference(&after, &fresh(cx, &next)),
        None,
        "the edit differs from a fresh frame"
    );
}
