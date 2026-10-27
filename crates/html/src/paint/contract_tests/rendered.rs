//! Пробы готового документа: однократная краска реального HTML и видимые сплошные квады
//! в порядке краски (граница ∩ маска, после переноса). Для unit-тестов потребителей.

use gpui::{
    AvailableSpace, Bounds, Hsla, IntoElement, ParentElement, Pixels, Scene, TestAppContext,
    canvas, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

/// Visible solid quads of `html` laid out in a `w × h` viewport, in paint order.
pub(crate) fn quads(html: &str, w: f32, h: f32) -> Vec<(Bounds<Pixels>, Hsla)> {
    let document = crate::Document::builder(html, crate::BROWSER_CSS)
        .build()
        .expect("test document");
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (w, h),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let boxes = document.render(&options);
    let out = Rc::new(RefCell::new(Vec::new()));
    let sink = out.clone();
    let mut cx = TestAppContext::single();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(w), px(h)), move |_, _| {
            let root = RefCell::new(Some(gpui::div().children(boxes).into_any_element()));
            canvas(
                move |_, window, cx| {
                    let mut root = root.borrow_mut().take().expect("one prepaint");
                    root.layout_as_root(
                        size(
                            AvailableSpace::Definite(px(w)),
                            AvailableSpace::Definite(px(h)),
                        ),
                        window,
                        cx,
                    );
                    root.prepaint_at(point(px(0.), px(0.)), window, cx);
                    root
                },
                move |_, mut root, window, cx| {
                    let extent = Bounds::new(point(px(0.), px(0.)), size(px(w), px(h)));
                    let (_, record) = window
                        .record_paint(extent, cx, |window, cx| root.paint(window, cx))
                        .expect("document paint");
                    collect(
                        record.scene(),
                        window.scale_factor(),
                        &mut sink.borrow_mut(),
                    );
                },
            )
        });
    out.take()
}

fn collect(scene: &Scene, scale: f32, out: &mut Vec<(Bounds<Pixels>, Hsla)>) {
    let mut pending = vec![scene];
    while let Some(scene) = pending.pop() {
        for quad in &scene.quads {
            let Some(color) = quad.background.as_solid().filter(|c| c.a > 0.) else {
                continue;
            };
            let t = quad.transformation.translation;
            let mut b = quad.bounds;
            b.origin.x.0 += t[0];
            b.origin.y.0 += t[1];
            let visible = b.intersect(&quad.content_mask.bounds);
            if visible.size.width.0 <= 0. || visible.size.height.0 <= 0. {
                continue;
            }
            let logical = Bounds::new(
                point(
                    px(visible.origin.x.0 / scale),
                    px(visible.origin.y.0 / scale),
                ),
                size(
                    px(visible.size.width.0 / scale),
                    px(visible.size.height.0 / scale),
                ),
            );
            out.push((logical, color));
        }
        pending.extend(scene.groups.iter().rev().map(|group| &group.scene));
    }
}

/// Colour of the last painted quad covering `(x, y)`.
pub(crate) fn top(quads: &[(Bounds<Pixels>, Hsla)], x: f32, y: f32) -> Option<Hsla> {
    quads
        .iter()
        .rev()
        .find(|(b, _)| b.contains(&point(px(x), px(y))))
        .map(|(_, c)| *c)
}

#[test]
fn p08dbg_probe() {
    let html =
        std::fs::read_to_string(std::env::var("P08_HTML").unwrap_or_default()).unwrap_or_default();
    if html.is_empty() {
        return;
    }
    for (b, c) in quads(&html, 800., 600.) {
        eprintln!("P08Q {:?} {:?} {:?}", b.origin, b.size, (c.h, c.s, c.l));
    }
}
