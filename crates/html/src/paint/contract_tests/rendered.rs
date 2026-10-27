//! Пробы готового документа: однократная краска реального HTML и видимые сплошные квады
//! в порядке краски (граница ∩ маска, после переноса). Для unit-тестов потребителей.

use gpui::{
    AnyElement, AvailableSpace, Bounds, Context, Hsla, IntoElement, ParentElement, Pixels, Render,
    Scene, Styled, TestAppContext, Window, canvas, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

type Quads = Rc<RefCell<Vec<(Bounds<Pixels>, Hsla)>>>;

/// One explicit frame inside a real view (element state needs a current view).
struct Frame {
    root: Rc<RefCell<Option<AnyElement>>>,
    sink: Quads,
    size: (f32, f32),
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (root, sink, (w, h)) = (self.root.clone(), self.sink.clone(), self.size);
        canvas(
            move |_, window, cx| {
                let mut root = root.borrow_mut().take()?;
                root.layout_as_root(
                    size(
                        AvailableSpace::Definite(px(w)),
                        AvailableSpace::Definite(px(h)),
                    ),
                    window,
                    cx,
                );
                root.prepaint_at(point(px(0.), px(0.)), window, cx);
                Some(root)
            },
            move |_, root, window, cx| {
                let Some(mut root) = root else { return };
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
        .size_full()
    }
}

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
    let root = Rc::new(RefCell::new(None));
    let sink: Quads = Rc::default();
    let mut cx = TestAppContext::single();
    let (_, vcx) = cx.add_window_view(|_, _| Frame {
        root: root.clone(),
        sink: sink.clone(),
        size: (w, h),
    });
    sink.borrow_mut().clear();
    *root.borrow_mut() = Some(
        gpui::div()
            .children(document.render(&options))
            .into_any_element(),
    );
    vcx.update(|window, app| {
        window.refresh();
        window.draw(app).clear(app);
    });
    sink.take()
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
    let list = std::env::var("P08_HTML").unwrap_or_default();
    for path in list.split(';').filter(|p| !p.is_empty()) {
        let html = std::fs::read_to_string(path).unwrap_or_default();
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        for (b, c) in quads(&html, 800., 600.) {
            eprintln!(
                "P08Q {name} {:?} {:?} {:?}",
                b.origin,
                b.size,
                (c.h, c.s, c.l)
            );
        }
    }
}
