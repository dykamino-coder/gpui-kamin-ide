//! A class that changes the SVG paints `fill` and `stroke` of a box
//! (`Computed::same_layout`, `revision::paint_damage`): on an HTML box they
//! paint nothing, so the revision keeps the frame and writes the rest of the
//! change into the scene layer; on a shape in an `svg` the frame is built
//! with the new paint. Either frame equals a fresh document's. A `class` or
//! `data-*` on a shape reaches it through the cascade only: a box value
//! (`background`), which SVG content does not paint, keeps the frame on the
//! screen and off it; a geometry attribute builds the frame.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::{TestAppContext, VisualTestContext, px, size};

fn window(cx: &mut VisualTestContext, height: f32) {
    cx.simulate_resize(size(px(400.), px(height)));
    frame(cx);
}

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

/// 120 rows of 20px and 20 `svg`s of shapes; `edit` goes into the start tag
/// of the `rect` of `svg` 10, below the rows (`below`) or above them.
fn shapes(edit: &str, below: bool) -> String {
    let svgs: String = (0..20)
        .map(|i| {
            let e = if i == 10 { edit } else { "" };
            format!(
                "<svg width=80 height=60><rect{e} x=5 y=5 width=65 height=45 fill=#397 /></svg>"
            )
        })
        .collect();
    let rows: String = (0..120)
        .map(|i| format!("<div class=r>row {i}</div>"))
        .collect();
    let (top, bottom) = if below { (rows, svgs) } else { (svgs, rows) };
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         .r{{height:20px}} svg{{vertical-align:top}} [data-edit],.hl{{background:#eee}}\
         .f{{fill:#e00}} .s{{stroke:#00e}} .o{{opacity:.5}} .t{{transform:translate(5px)}}\
         .n{{display:none}} body:has(rect.h) .r:nth-child(odd){{background:#eee}}\
         svg:has(.h) ~ .r{{color:#e00}}</style></head><body>{top}{bottom}</body></html>"
    )
}

/// The painted primitives in paint order: quads and every other kind (the
/// `svg` images) with their bounds, without the atlas tile an image took
/// (each document allocates its own; the pixels are the benches' check).
fn scene(cx: &mut VisualTestContext) -> (Vec<String>, Vec<String>) {
    let sequence = cx.update(|window, _| window.painted_sequence());
    let untiled = |k: &str| match (k.find(", tile: "), k.find(", transformation: ")) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &k[..a], &k[b..]),
        _ => k.to_owned(),
    };
    let sequence = sequence
        .iter()
        .map(|(k, b)| format!("{} {b:?}", untiled(k)))
        .collect();
    (quads(cx), sequence)
}

/// The edit `""` to `edit` of `shapes` in a 300px window: whether it kept the
/// frame, the scene after it and in a window showing everything; the same
/// two scenes of a fresh document.
#[allow(clippy::type_complexity)]
fn shape_edit(
    cx: &mut TestAppContext,
    edit: &str,
    below: bool,
) -> (
    bool,
    [(Vec<String>, Vec<String>); 2],
    [(Vec<String>, Vec<String>); 2],
) {
    let (view, vcx) = settled(cx, &shapes("", below), &shapes(" data-x", below));
    window(vcx, 300.);
    update(&view, vcx, &shapes("", below));
    update(&view, vcx, &shapes("", below));
    let before = frames(&view, vcx);
    update(&view, vcx, &shapes(edit, below));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let now = scene(vcx);
    window(vcx, 4200.);
    let edited = [now, scene(vcx)];
    let (_, fresh) = open(cx, &shapes(edit, below));
    window(fresh, 300.);
    let first = scene(fresh);
    window(fresh, 4200.);
    (kept, edited, [first, scene(fresh)])
}

#[gpui::test]
fn a_box_value_on_a_shape_below_keeps_the_frame_and_shows_as_fresh(cx: &mut TestAppContext) {
    // `background` does not paint SVG content: the `svg` image stays.
    for edit in [" data-edit", " class=hl"] {
        let (kept, edited, fresh) = shape_edit(cx, edit, true);
        assert!(kept, "{edit}: the frame was not kept");
        assert_eq!(edited, fresh, "{edit}: a frame differs from a fresh one");
    }
}

#[gpui::test]
fn a_box_value_on_a_shape_on_the_screen_keeps_the_frame_as_fresh(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = shape_edit(cx, " data-edit", false);
    assert!(kept, "the frame was not kept");
    assert_eq!(edited, fresh, "a frame differs from a fresh one");
}

#[gpui::test]
fn a_geometry_or_presentation_attribute_of_a_shape_builds_its_frame_as_fresh(
    cx: &mut TestAppContext,
) {
    // The `svg` image reads these as they are: the revision is built.
    for edit in [
        " rx=9",
        " fill=#f00",
        " width=20",
        " transform=translate(5)",
    ] {
        for below in [true, false] {
            let (kept, edited, fresh) = shape_edit(cx, edit, below);
            assert!(!kept, "{edit}: the frame was kept");
            assert_eq!(edited, fresh, "{edit}: a frame differs from a fresh one");
        }
    }
}

#[gpui::test]
fn a_class_that_changes_how_a_shape_paints_builds_its_frame_as_fresh(cx: &mut TestAppContext) {
    // Fill, stroke, opacity, transform, display: the image changes.
    for class in ["f", "s", "o", "t", "n"] {
        for below in [true, false] {
            let (kept, edited, fresh) = shape_edit(cx, &format!(" class={class}"), below);
            assert!(!kept, "{class}: the frame was kept");
            assert_eq!(edited, fresh, "{class}: a frame differs from a fresh one");
        }
    }
}

#[gpui::test]
fn html_boxes_whose_selectors_read_a_shape_class_show_as_fresh(cx: &mut TestAppContext) {
    // `:has` and a sibling combinator reach HTML boxes outside the `svg`.
    for below in [true, false] {
        let (_, edited, fresh) = shape_edit(cx, " class=h", below);
        assert_eq!(edited, fresh, "a frame differs from a fresh one");
    }
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
