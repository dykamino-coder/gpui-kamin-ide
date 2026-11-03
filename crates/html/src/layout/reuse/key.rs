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
    pub(crate) fn read(cx: &LayoutCx) -> Self {
        Self {
            keys: cx.deferred.keys.get(),
            anchors: crate::layout::positioned::anchor::seq(&cx.layout().anchors),
        }
    }

    pub(crate) fn write(self, cx: &LayoutCx) {
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
        self.same_except_width(other)
            && self.constraints == other.constraints
            && self.viewport == other.viewport
    }

    /// Тот же поток, кроме инлайн-размера окна и содержащего блока: так
    /// меняется ключ при смене ширины окна (Blink `ConstraintSpace` с другим
    /// `AvailableSize().inline_size`). Ребёнку такого потока подходит запись,
    /// только если его раскладка от этой ширины не зависит (`Width`).
    pub(super) fn same_except_width(&self, other: &Self) -> bool {
        *self.root == *other.root
            && same_boxes(&self.html, &other.html)
            && same_boxes(&self.body, &other.body)
            && self.constraints.avail_h == other.constraints.avail_h
            && self.constraints.cell_bfc == other.constraints.cell_bfc
            && self.constraints.cb_width.is_some() == other.constraints.cb_width.is_some()
            && self.constraints.avail_w.is_some() == other.constraints.avail_w.is_some()
            && self.viewport.1 == other.viewport.1
            && self.text == other.text
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }
}
