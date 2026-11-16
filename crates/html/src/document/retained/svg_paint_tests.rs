//! A class that changes the SVG paints `fill` and `stroke` of a box
//! (`Computed::same_layout`, `revision::paint_damage`): on an HTML box they
//! paint nothing, so the revision keeps the frame and writes the rest of the
//! change into the scene layer; on a shape in an `svg` the frame is built
//! with the new paint. Either frame equals a fresh document's.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// Rows of 20px; `class` goes on row 3, a `div`, and on the `rect` of the
/// `svg` after it.
fn page(class: &str) -> String {
    let rows: String = (0..40)
        .map(|i| {
            let c = if i == 3 { class } else { "" };
            format!("<div class='r {c}'>row {i}</div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         .r{{height:20px}} .hl{{background:#eee;fill:#e00;stroke:#00e}} \
         .paint{{fill:#0a0;stroke:#a0a}}</style></head><body>{rows}\
         <svg width=40 height=20><rect class='{class}' width=30 height=10 /></svg></body></html>"
    )
}

fn frames(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.retained_frames())
}

fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|mut q| {
            q.order = 0;
            format!("{q:?}")
        });
        quads.collect()
    })
}

/// Whether `class` on a settled document kept its frame, its frame and a
/// fresh document's.
fn edit(cx: &mut TestAppContext, class: &str) -> (bool, Vec<String>, Vec<String>) {
    let (view, vcx) = settled(cx, &page(""), &page("x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(class));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &page(class));
    frame(fresh);
    (kept, edited, quads(fresh))
}

#[gpui::test]
fn fill_and_stroke_beside_a_background_show_as_fresh(cx: &mut TestAppContext) {
    // The `rect` takes the class too: its `svg` is built, the frame with it.
    let (_, edited, fresh) = edit(cx, "hl");
    assert_eq!(edited, fresh, "the edited frame differs from a fresh one");
}

#[gpui::test]
fn fill_and_stroke_of_a_shape_build_its_frame_as_fresh(cx: &mut TestAppContext) {
    let (_, edited, fresh) = edit(cx, "paint");
    assert_eq!(
        edited, fresh,
        "the shape's new paint differs from a fresh one"
    );
}

#[gpui::test]
fn fill_on_an_html_box_alone_keeps_the_frame(cx: &mut TestAppContext) {
    // The `rect` keeps every attribute: a change of one rasters its `svg`.
    let html = |class: &str| page(class).replace(&format!("<rect class='{class}'"), "<rect ");
    let (view, vcx) = settled(cx, &html(""), &html("x"));
    update(&view, vcx, &html(""));
    update(&view, vcx, &html(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &html("paint"));
    let after = frames(&view, vcx);
    assert_eq!(after, (before.0, before.1 + 1), "the frame was not kept");
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &html("paint"));
    frame(fresh);
    assert_eq!(
        edited,
        quads(fresh),
        "the kept frame differs from a fresh one"
    );
}
