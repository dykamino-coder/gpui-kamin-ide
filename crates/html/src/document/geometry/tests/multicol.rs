//! Дробные срезы layout и переполнение поперёк колонок не зависят от paint-маски.

use super::*;
use crate::Rect;

#[gpui::test]
fn geometry_multicol_preserves_fractional_fragments_and_inline_overflow(cx: &mut TestAppContext) {
    for width in [100.0, 160.0] {
        let html = format!(
            "<style>body{{margin:0}}</style><div style='margin-left:.25px;margin-top:.375px;columns:2;column-gap:20.5px;width:221px;height:100.25px;column-fill:auto'><div id='split' style='width:{width}px;height:180.5px'></div></div>"
        );
        let elements = snapshot(cx, &html, 400.);
        let e = find(&elements, "split");
        assert_eq!(e.client_rects.len(), 2, "{:?}", e.client_rects);
        let a = e.client_rects[0];
        let b = e.client_rects[1];
        assert_eq!(a.x, 0.25);
        assert_eq!(a.y, 0.375);
        assert_eq!(a.width, width);
        assert_eq!(b.width, width);
        assert_eq!(a.height, 100.25);
        assert_eq!(b.height, 80.25);
        assert_eq!(b.x - a.x, 120.75);
    }
}

#[gpui::test]
fn geometry_multicol_applies_transform_to_fragment_quads(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div style='columns:2;column-gap:20px;width:220px;height:100px;column-fill:auto'><div id='split' style='height:180px;transform:scale(2);transform-origin:0 0'></div></div>",
        400.,
    );
    let rects = &find(&elements, "split").client_rects;
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_eq!(rects[0].width, 200.);
    assert_eq!(rects[0].height, 200.);
    assert_eq!(rects[1].x, 240., "{rects:?}");
    assert_eq!(rects[1].y, 0., "{rects:?}");
    assert_eq!(rects[1].height, 160.);
}

#[gpui::test]
fn geometry_multicol_keeps_fractional_absolute_and_nested_transform(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}.columns{margin-left:.25px;margin-top:.375px;columns:2;column-gap:20.5px;width:221px;height:100.25px;column-fill:auto}.owner{position:relative;height:180.5px}.absolute{position:absolute;left:0;top:0;width:50px;height:180.5px}</style><div class='columns'><div class='owner'><div id='absolute' class='absolute'><div id='nested' style='width:20px;height:170.5px;transform:translate(3.25px,4.5px)'></div></div></div></div>",
        400.,
    );
    for (id, x, y, width, tail) in [
        ("absolute", 0.25, 0.375, 50., 80.25),
        ("nested", 3.5, 4.875, 20., 70.25),
    ] {
        let rects = &find(&elements, id).client_rects;
        assert_eq!(rects.len(), 2, "{id}: {rects:?}");
        assert_eq!(
            rects[0],
            Rect {
                x,
                y,
                width,
                height: 100.25
            },
            "{id}"
        );
        assert_eq!(
            rects[1],
            Rect {
                x: x + 120.75,
                y,
                width,
                height: tail
            },
            "{id}"
        );
    }
}

#[gpui::test]
fn geometry_multicol_keeps_zero_height_box_in_its_fragment(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div style='columns:2;column-gap:20px;width:220px;height:100px;column-fill:auto'><div style='height:20px'></div><div id='empty' style='width:40px;height:0'></div><div style='height:160px'></div></div>",
        400.,
    );
    let e = find(&elements, "empty");
    assert_eq!(e.client_rects.len(), 1, "{:?}", e.client_rects);
    assert_eq!(e.border_box.x, 0.);
    assert_eq!(e.border_box.y, 20.);
    assert_eq!(e.border_box.width, 40.);
    assert_eq!(e.border_box.height, 0.);
}
