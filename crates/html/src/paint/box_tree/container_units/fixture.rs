//! Реальный HTML layout: размер контейнера публикуется до финальной краски.
use crate::dom::{Element, Node};
use crate::{Document, RenderOpts};
use gpui::{Context, ParentElement, Render, Styled, Window, div, px};
use std::cell::Cell;
use std::rc::Rc;

struct Fixture {
    doc: Rc<Document>,
    width: Rc<Cell<f32>>,
    renders: Rc<Cell<usize>>,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        self.renders.set(self.renders.get() + 1);
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (self.width.get(), 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        div()
            .block()
            .w(px(self.width.get()))
            .h(px(600.))
            .children(self.doc.render(&opts))
    }
}

pub(super) fn element<'a>(nodes: &'a [Node], id: &str) -> &'a Element {
    fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.attr("id") == Some(id) => Some(e),
            Node::Element(e) => find(&e.children, id),
            _ => None,
        })
    }
    find(nodes, id).unwrap_or_else(|| panic!("missing #{id}"))
}

pub(super) fn content(doc: &Document, id: &str) -> (f32, f32) {
    let c = element(doc.nodes(), id).style.computed();
    (
        c.cq_size.0.expect("settled width"),
        c.cq_size.1.expect("settled height"),
    )
}

pub(super) fn run(
    cx: &mut gpui::TestAppContext,
    html: &str,
    check: impl FnOnce(&Document, &Rc<Cell<f32>>, &Rc<Cell<usize>>, &mut gpui::VisualTestContext),
) {
    let doc = Rc::new(Document::builder(html, "").build());
    let width = Rc::new(Cell::new(400.));
    let renders = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, _| Fixture {
        doc: doc.clone(),
        width: width.clone(),
        renders: renders.clone(),
    });
    cx.update(|window, app| {
        window.draw(app).clear(app);
    });
    check(&doc, &width, &renders, cx);
}
