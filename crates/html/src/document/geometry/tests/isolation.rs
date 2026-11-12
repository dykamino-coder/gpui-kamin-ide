//! Вложенный запрос не наследует идентичность и шрифты внешнего элемента.

use super::*;
use crate::LineHeight;
use gpui::{TextStyle, TextStyleRefinement, px};

#[gpui::test]
fn geometry_table_keeps_caption_separate_from_the_grid(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}table{border-spacing:0;width:100px}caption{height:20px}td{height:30px;padding:0}</style><table id='table'><caption id='caption'>title</caption><tr><td>cell</td></tr></table>",
        300.,
    );
    let table = find(&elements, "table");
    let caption = find(&elements, "caption");
    assert_eq!(table.client_rects.len(), 2, "{:?}", table.client_rects);
    assert_eq!(table.client_rects[1], caption.border_box);
    assert!(table.client_rects[0].y >= caption.border_box.y + caption.border_box.height);
}

#[gpui::test]
fn geometry_svg_excludes_markers_and_paint_effects(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><svg width='200' height='100'><defs><marker id='marker' markerWidth='100' markerHeight='100' refX='0' refY='0'><rect width='100' height='100'/></marker><clipPath id='clip'><rect width='1' height='1'/></clipPath></defs><g id='group'><path id='shape' d='M10 10 L30 20' stroke='black' stroke-width='12' marker-end='url(#marker)' clip-path='url(#clip)'/></g></svg>",
        300.,
    );
    let shape = find(&elements, "shape");
    assert_eq!(shape.client_rects.len(), 1);
    assert_eq!(shape.border_box.width, 20.);
    assert_eq!(shape.border_box.height, 10.);
    assert_eq!(find(&elements, "group").border_box, shape.border_box);
}

#[gpui::test]
fn geometry_svg_text_content_elements_keep_glyph_cell_boxes(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><svg width='200' height='100'><text id='text' x='10' y='40' font-family='monospace' font-size='20'>a<tspan id='span'>bc</tspan>d</text></svg>",
        300.,
    );
    let text = find(&elements, "text");
    let span = find(&elements, "span");
    assert_eq!(text.client_rects.len(), 1);
    assert_eq!(span.client_rects.len(), 1);
    assert!(span.border_box.width > 0.);
    assert!(span.border_box.x > text.border_box.x);
    assert!(span.border_box.width < text.border_box.width);
    assert_eq!(span.border_box.height, text.border_box.height);
}

#[gpui::test]
fn geometry_svg_uses_object_boxes_and_viewbox_transforms(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><svg width='200' height='100' viewBox='0 0 100 50'><g id='group' transform='translate(3 4)'><rect id='shape' x='5' y='6' width='20' height='10' stroke='red' stroke-width='12'/></g></svg>",
        300.,
    );
    let shape = find(&elements, "shape");
    assert_eq!(shape.client_rects.len(), 1);
    assert_eq!(shape.border_box.width, 40.);
    assert_eq!(shape.border_box.height, 20.);
    assert_eq!(shape.border_box.x, 16.);
    assert_eq!(shape.border_box.y, 20.);
    assert_eq!(find(&elements, "group").border_box, shape.border_box);
}

#[gpui::test]
fn geometry_upright_inline_keeps_its_box_and_computed_line_height(cx: &mut TestAppContext) {
    for contents in ["abcd", "a<b style='font-size:30px'>bc</b>d"] {
        let html = format!(
            "<style>body{{margin:0}}div{{writing-mode:vertical-rl;text-orientation:upright;font:20px/30px monospace;height:200px}}</style><div><span id='vertical'>{contents}</span></div>"
        );
        let elements = snapshot(cx, &html, 300.);
        let element = find(&elements, "vertical");
        assert_eq!(element.client_rects.len(), 1);
        assert!(element.border_box.height > element.border_box.width);
        assert_eq!(element.font_size, 20.);
        assert_eq!(element.line_height, LineHeight::Pixels(30.));
        if contents.contains("<b") {
            let child = elements.iter().find(|e| e.tag == "b").unwrap();
            assert_eq!(child.client_rects.len(), 1);
            assert_eq!(child.border_box.height, 60.);
        } else {
            assert_eq!(element.border_box.height, 80.);
        }
    }
}

#[gpui::test]
fn geometry_native_vertical_inline_uses_the_physical_axis(cx: &mut TestAppContext) {
    for mode in ["vertical-rl", "vertical-lr", "sideways-lr"] {
        let html = format!(
            "<style>body{{margin:0}}div{{writing-mode:{mode};font:20px/30px monospace;height:200px}}span{{padding:0;border:0}}</style><div><span id='vertical'>abcd</span></div>"
        );
        let elements = snapshot(cx, &html, 300.);
        let e = find(&elements, "vertical");
        assert_eq!(e.client_rects.len(), 1, "{mode}: {:?}", e.client_rects);
        assert!(
            e.border_box.height > e.border_box.width,
            "{mode}: {:?}",
            e.border_box
        );
        let moved = html.replace(
            "height:200px",
            "height:200px;margin-left:17.25px;margin-top:13.5px",
        );
        let translated = snapshot(cx, &moved, 300.);
        let translated = find(&translated, "vertical").border_box;
        assert_eq!(translated.width, e.border_box.width, "{mode}");
        assert_eq!(translated.height, e.border_box.height, "{mode}");
        assert_eq!(translated.x - e.border_box.x, 17.25, "{mode}");
        assert_eq!(translated.y - e.border_box.y, 13.5, "{mode}");
    }
}

#[gpui::test]
fn geometry_query_isolates_identity_and_inherited_text(cx: &mut TestAppContext) {
    let (_, cx) = cx
        .add_window_view(|_, _| View(Rc::new(Document::builder("", BROWSER_CSS).build().unwrap())));
    cx.update(|window, cx| {
        window.with_layout_query(cx, |window, cx| {
            window.with_id("outer", |window| {
                window.with_text_style(
                    Some(TextStyleRefinement {
                        font_size: Some(px(71.).into()),
                        ..Default::default()
                    }),
                    |window| {
                        let outer = window.text_style().font_size;
                        window.with_layout_query(cx, |window, _| {
                            assert_eq!(
                                window.text_style().font_size,
                                TextStyle::default().font_size
                            );
                            window.with_global_id("inner".into(), |id, _| assert_eq!(id.len(), 1));
                        });
                        assert_eq!(window.text_style().font_size, outer);
                        window.with_global_id("inner".into(), |id, _| assert_eq!(id.len(), 2));
                    },
                );
            });
        });
    });
}
