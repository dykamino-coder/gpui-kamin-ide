//! Пробы границы paint policy движка: порядок, однократность фаз и снимки GPUI.
//! Проверяют существующие consumers без переноса CSS-правил в новый механизм.

mod capture;
mod order;
mod scope;
mod snapshot_abort;
mod snapshot_scope;
mod snapshot_unwind;
mod state;

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, Bounds, LayoutId, ParentElement, Pixels, Window, div, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

type Events = Rc<RefCell<Vec<(&'static str, &'static str)>>>;

struct Probe {
    name: &'static str,
    events: Events,
}

impl Hooks for Probe {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.events.borrow_mut().push(("layout", self.name));
        (window.request_layout(Default::default(), [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {
        self.events.borrow_mut().push(("prepaint", self.name));
    }

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {
        self.events.borrow_mut().push(("paint", self.name));
    }
}

fn probe(name: &'static str, events: &Events) -> AnyElement {
    Probe {
        name,
        events: events.clone(),
    }
    .hooked()
}

fn collect(children: Vec<AnyElement>) -> gpui::Div {
    let (open, close) = gpui::PaintCollect::pair();
    // Контент входит в Div уже после открытия collector: его fallback late pass
    // не должен пересекать закрывающий marker внешнего контейнера.
    div()
        .child(open)
        .child(div().children(children))
        .child(close)
}

fn draw<E: gpui::Element>(make: impl FnOnce() -> E) {
    gpui::paint_collect_reset();
    let mut cx = gpui::TestAppContext::single();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            // AnyElement должен создаваться внутри app-specific arena этого draw,
            // иначе fallback arena потока удерживает дерево до завершения потока.
            make()
        });
    gpui::paint_collect_reset();
    cx.quit();
}

fn assert_phases(events: &Events, tree_order: &[&str], paint_order: &[&str]) {
    for (phase, expected) in [
        ("layout", tree_order),
        ("prepaint", tree_order),
        ("paint", paint_order),
    ] {
        let actual: Vec<_> = events
            .borrow()
            .iter()
            .filter_map(|&(p, name)| (p == phase).then_some(name))
            .collect();
        assert_eq!(actual, expected, "{phase}");
    }
}
