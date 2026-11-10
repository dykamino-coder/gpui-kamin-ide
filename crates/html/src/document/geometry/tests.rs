//! Публичный запрос проверяется на настоящем дереве layout и prepaint.

use crate::{BROWSER_CSS, Document, ElementGeometry, RenderOpts};
use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};
use std::rc::Rc;

mod isolation;

struct View(Rc<Document>);

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn snapshot(cx: &mut TestAppContext, html: &str, width: f32) -> Vec<ElementGeometry> {
    let doc = Document::builder(html, BROWSER_CSS).build().unwrap();
    let (view, cx) = cx.add_window_view(|_, _| View(Rc::new(doc)));
    cx.update(|window, cx| {
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (width, 300.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        let doc = view.read(cx).0.clone();
        let first = doc.element_geometry(&opts, window, cx).unwrap();
        let second = doc.element_geometry(&opts, window, cx).unwrap();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.client_rects, b.client_rects, "repeat query: {}", a.id);
        }
        first
    })
}

fn find<'a>(elements: &'a [ElementGeometry], id: &str) -> &'a ElementGeometry {
    elements.iter().find(|e| e.id == id).unwrap()
}

#[gpui::test]
fn geometry_box_edges_and_boxless_elements(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div id='box' style='width:80px;height:30px;padding:7px;border:3px solid'></div><div id='none' style='display:none'></div>",
        300.,
    );
    let b = find(&elements, "box");
    assert_eq!(b.client_rects.len(), 1);
    assert_eq!(b.border_box.width, 100.);
    assert_eq!(b.border_box.height, 50.);
    assert_eq!(b.padding_boxes[0].width, 94.);
    assert_eq!(b.content_boxes[0].width, 80.);
    assert_eq!(b.content_boxes[0].x - b.border_box.x, 10.);
    assert!(find(&elements, "none").client_rects.is_empty());
}

#[gpui::test]
fn geometry_boxless_descendants_keep_computed_font_metrics(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<div style='display:none;font-size:32px;line-height:48px'><span id='hidden'>text</span></div>",
        300.,
    );
    let hidden = find(&elements, "hidden");
    assert!(hidden.client_rects.is_empty());
    assert_eq!(hidden.font_size, 32.);
    assert_eq!(hidden.line_height, crate::LineHeight::Pixels(48.));
}

#[gpui::test]
fn geometry_inline_keeps_each_line_fragment(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0;font:20px/30px monospace}</style><div style='width:90px'><span id='inline'>aaaa bbbb cccc dddd eeee</span></div>",
        300.,
    );
    let e = find(&elements, "inline");
    assert!(e.client_rects.len() >= 3, "{:?}", e.client_rects);
    assert!(e.client_rects.windows(2).all(|r| r[0].y < r[1].y));
    assert!(e.client_rects.iter().all(|r| r.height < 30.));
    assert_eq!(e.padding_boxes.len(), e.client_rects.len());
}

#[gpui::test]
fn geometry_nested_inline_and_empty_box_keep_fragment_identity(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0;font:20px/30px monospace}div{width:200px;text-align:right}</style><div><span id='outer'>a<b>b</b>c</span><span id='empty'></span></div>",
        300.,
    );
    let outer = find(&elements, "outer");
    let empty = find(&elements, "empty");
    assert_eq!(outer.client_rects.len(), 1, "{:?}", outer.client_rects);
    assert_eq!(empty.client_rects.len(), 1, "{:?}", empty.client_rects);
    assert_eq!(empty.border_box.width, 0.);
    assert!(empty.border_box.height > 0.);
    assert!(outer.border_box.x > 0.);
    assert_eq!(
        empty.border_box.x,
        outer.border_box.x + outer.border_box.width
    );
}

#[gpui::test]
fn geometry_reports_computed_font_before_size_adjustment(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<div id='font' style='font-size:20px;font-size-adjust:1;line-height:1.5'>x</div>",
        300.,
    );
    let e = find(&elements, "font");
    assert_eq!(e.font_size, 20.);
    assert_eq!(e.line_height, crate::LineHeight::Pixels(30.));
}

#[gpui::test]
fn geometry_percentage_height_uses_the_initial_containing_block(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>html,body{margin:0;height:100%}</style><div id='half' style='height:50%;width:20px'></div>",
        300.,
    );
    assert_eq!(find(&elements, "half").border_box.height, 150.);
}

#[gpui::test]
fn geometry_hidden_subtree_counters_do_not_affect_visible_layout(cx: &mut TestAppContext) {
    let page = |extra| {
        format!(
            "<style>body{{counter-reset:n;font:20px monospace}}#box{{display:inline-block}}#box::before{{counter-increment:n;content:counter(n)}}</style>{extra}<div id='box'></div>"
        )
    };
    let plain = snapshot(cx, &page(""), 300.);
    let hidden = snapshot(
        cx,
        &page(
            "<div style='display:none;counter-increment:n 999'><span style='counter-increment:n 999'></span></div>",
        ),
        300.,
    );
    assert_eq!(
        find(&plain, "box").border_box,
        find(&hidden, "box").border_box
    );
}

#[gpui::test]
fn geometry_ancestor_and_singular_transforms(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div style='transform:translate(20px,30px)'><div id='box' style='width:40px;height:10px;transform:scaleX(0);transform-origin:0 0'></div></div>",
        300.,
    );
    let b = find(&elements, "box").border_box;
    assert_eq!(b.x, 20.);
    assert_eq!(b.y, 30.);
    assert_eq!(b.width, 0.);
    assert_eq!(b.height, 10.);
}

#[gpui::test]
fn geometry_multicol_block_has_continuations(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div style='columns:2;column-gap:20px;width:220px;height:100px;column-fill:auto'><div id='split' style='height:180px;background:green'></div></div>",
        300.,
    );
    let e = find(&elements, "split");
    assert_eq!(e.client_rects.len(), 2, "{:?}", e.client_rects);
    assert_eq!(e.client_rects[0].width, 100.);
    assert_eq!(e.client_rects[0].height, 100.);
    assert_eq!(e.client_rects[1].x - e.client_rects[0].x, 120.);
    assert_eq!(e.client_rects[1].height, 80.);
}

#[gpui::test]
fn geometry_query_restores_outer_pass_after_unwind(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| {
        View(Rc::new(
            Document::builder("<div></div>", BROWSER_CSS)
                .build()
                .unwrap(),
        ))
    });
    cx.update(|window, cx| {
        window.with_layout_query(cx, |window, cx| {
            window.with_element_offset(gpui::point(gpui::px(17.), gpui::px(23.)), |window| {
                let offset = window.element_offset();
                window.with_render_context(Some(Rc::new(37_u32)), |window| {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        window.with_layout_query(cx, |window, _| {
                            assert_eq!(
                                window.element_offset(),
                                gpui::point(gpui::px(0.), gpui::px(0.))
                            );
                            assert!(window.render_context::<u32>().is_none());
                            panic!("query failure");
                        });
                    }));
                    assert!(result.is_err());
                    assert_eq!(window.element_offset(), offset);
                    assert_eq!(*window.render_context::<u32>().unwrap(), 37);
                });
            });
        });
        window.with_layout_query(cx, |window, _| {
            assert_eq!(
                window.element_offset(),
                gpui::point(gpui::px(0.), gpui::px(0.))
            );
        });
    });
}
