//! Ключ корневого потока и счётчики сборки, которые продвигает его ребёнок.

use crate::dom::{Children, Element};
use crate::layout::cx::{BoxConstraints, LayoutCx};
use crate::style::computed::BoxStyle;

/// Счётчики сборки, которые поддерево продвигает: ключи краски и якорей.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Counters {
    keys: u64,
    anchors: u32,
}

impl Counters {
    pub(super) fn read(cx: &LayoutCx) -> Self {
        Self {
            keys: cx.deferred.keys.get(),
            anchors: crate::layout::positioned::anchor::seq(&cx.layout().anchors),
        }
    }

    pub(super) fn write(self, cx: &LayoutCx) {
        cx.deferred.keys.set(self.keys);
        crate::layout::positioned::anchor::set_seq(&cx.layout().anchors, self.anchors);
    }
}

/// Коробка элемента без детей: ключ сравнивает только её (`dom::same_box`),
/// а дети удержали бы дерево кадра записи.
pub(super) fn box_only(e: &Element) -> Element {
    Element {
        children: Children::default(),
        ..e.clone()
    }
}

/// Окружение корневого потока, от которого зависит раскладка его детей.
pub(crate) struct FlowKey {
    /// Стиль начального содержащего блока: в него сложены снятые обёртки.
    pub(super) root: BoxStyle,
    /// Коробки `html` и `body`, если они не сняты.
    pub(super) html: Option<Element>,
    pub(super) body: Option<Element>,
    pub(super) constraints: BoxConstraints,
    pub(super) viewport: (f32, f32),
    pub(super) text: gpui::TextStyle,
    pub(super) normal_line_height: f32,
    pub(super) fonts: u64,
}

fn same_boxes(a: &Option<Element>, b: &Option<Element>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => crate::dom::same_box(a, b),
        _ => false,
    }
}

impl FlowKey {
    pub(super) fn same(&self, other: &Self) -> bool {
        self.same_outside_widths(other) && self.same_widths(other)
    }

    /// Everything but the widths of the flow: the containing-block and
    /// available widths of its children and the viewport width.
    pub(super) fn same_outside_widths(&self, other: &Self) -> bool {
        let (a, b) = (&self.constraints, &other.constraints);
        *self.root == *other.root
            && same_boxes(&self.html, &other.html)
            && same_boxes(&self.body, &other.body)
            && a.avail_h == b.avail_h
            && a.cell_bfc == b.cell_bfc
            && self.viewport.1 == other.viewport.1
            && self.text == other.text
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }

    /// The widths of the flow (see `same_outside_widths`).
    pub(super) fn same_widths(&self, other: &Self) -> bool {
        self.constraints.key_widths() == other.constraints.key_widths()
            && self.viewport.0 == other.viewport.0
    }
}
