//! Baseline probe for probes; split out to keep the owning module within 250 lines.

use crate::hooks::{Hooks, Wrap};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px, size};

/// Щуп базовой линии атома: пустой лист с базовой линией на своём верху.
/// В ряду `align-items: baseline` рядом с атомом его верх встаёт ровно на
/// базовую линию атома, и раскладка отдаёт её положением щупа. Для атома без
/// базовой линии taffy берёт нижний край полей (`flexbox.rs`) — у замещаемого
/// это и есть его базовая по §10.8.1.
pub(crate) struct BaselineProbe {
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Hooks for BaselineProbe {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, ()) {
        let id = window
            .request_measured_layout_with_baselines(gpui::Style::default(), |_, _, _, _| {
                (size(px(0.), px(0.)), Some(px(0.)), Some(px(0.)))
            });
        self.slot.set(Some(id));
        (id, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

/// Обёртка, запоминающая узел раскладки своего ребёнка: по нему берётся
/// ТОЧНЫЙ размер атома (`Window::layout_exact`) — округлённый к точке
/// устройства прибавлял до 0.4px на атом, и ряд атомов ровно в ширину строки
/// в неё уже не влезал (`c542-letter-sp-001-ref`, `c5505-mrgn-000`).
pub(crate) struct LayoutTap {
    pub(crate) child: AnyElement,
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Wrap for LayoutTap {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.slot.set(Some(id));
        id
    }
}
