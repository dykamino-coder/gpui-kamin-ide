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
    render_budget: usize,
    active: Rc<Cell<bool>>,
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        if !self.active.get() {
            return div();
        }
        self.renders.set(self.renders.get() + 1);
        assert!(
            self.renders.get() <= self.render_budget,
            "style/layout did not converge"
        );
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
    run_bounded(cx, html, usize::MAX, check);
}

pub(super) fn run_bounded(
    cx: &mut gpui::TestAppContext,
    html: &str,
    render_budget: usize,
    check: impl FnOnce(&Document, &Rc<Cell<f32>>, &Rc<Cell<usize>>, &mut gpui::VisualTestContext),
) {
    let doc = Rc::new(Document::builder(html, "").build());
    let width = Rc::new(Cell::new(400.));
    let renders = Rc::new(Cell::new(0));
    let active = Rc::new(Cell::new(false));
    let (_, cx) = cx.add_window_view(|_, _| Fixture {
        doc: doc.clone(),
        width: width.clone(),
        renders: renders.clone(),
        render_budget,
        active: active.clone(),
    });
    // Открытие окна рисует служебные кадры (включая appearance_changed).
    // Документ включается после них: счётчик измеряет один явный draw.
    active.set(true);
    cx.update(|window, app| {
        window.refresh();
        window.draw(app).clear(app);
    });
    check(&doc, &width, &renders, cx);
}
