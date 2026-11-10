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

/// The `html`/`body` boxes give the flow the same layout inputs. Their
/// paint-only values (`dom::same_layout_box`) do not reach a child's layout:
/// an inherited one (`color`) is in the child's own style, which its record
/// compares, and the others paint only these boxes, which every build builds.
fn same_boxes(a: &Option<Element>, b: &Option<Element>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => crate::dom::same_layout_box(a, b),
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
        // The root style folds the unwrapped `html`/`body` (see `same_boxes`).
        self.root.same_layout(&other.root)
            && (self.stripped(other)
                || same_boxes(&self.html, &other.html) && same_boxes(&self.body, &other.body))
            && a.avail_h == b.avail_h
            && a.cell_bfc == b.cell_bfc
            && self.viewport.1 == other.viewport.1
            && self.text == other.text
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }

    /// The `html`/`body` boxes of this key were compared with those of the
    /// next document at a rebuild (`Store::rebase`) and dropped, so the
    /// previous document's root styles do not live beside the new ones.
    /// Computed boxes are a function of the cascaded values, compared then,
    /// and of the document inputs this key still compares (viewport, fonts).
    pub(super) fn strip_boxes(&mut self) {
        self.html = None;
        self.body = None;
    }

    /// This key's boxes went at a rebuild (`strip_boxes`): a key of a flow
    /// below `body` always has its box.
    pub(super) fn stripped(&self, other: &Self) -> bool {
        self.body.is_none() && other.body.is_some()
    }

    /// The root style and boxes are equal in their paint-only values too.
    /// A stripped key (`stripped`) answers for its root style only: the
    /// rebuild that dropped its boxes knows how they compared.
    pub(super) fn same_paint(&self, other: &Self) -> bool {
        let boxes = |a: &Option<Element>, b: &Option<Element>| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => crate::dom::same_box(a, b),
            _ => false,
        };
        *self.root == *other.root
            && (self.stripped(other)
                || boxes(&self.html, &other.html) && boxes(&self.body, &other.body))
    }
    /// The widths of the flow (see `same_outside_widths`).
    pub(super) fn same_widths(&self, other: &Self) -> bool {
        self.constraints.key_widths() == other.constraints.key_widths()
            && self.viewport.0 == other.viewport.0
    }

    /// Тот же поток, кроме инлайн-размера окна и содержащего блока, оба
    /// по-прежнему известны: так меняется ключ при смене ширины окна (Blink
    /// `ConstraintSpace` с другим `AvailableSize().inline_size`). Ребёнку
    /// такого потока подходит запись, только если его раскладка от этой
    /// ширины не зависит (`Width`).
    pub(super) fn same_except_width(&self, other: &Self) -> bool {
        let (a, b) = (
            self.constraints.key_widths(),
            other.constraints.key_widths(),
        );
        self.same_outside_widths(other)
            && a.0.is_some() == b.0.is_some()
            && a.1.is_some() == b.1.is_some()
    }

    /// Ширина потока: доступная ширина его блоков `width: auto`.
    pub(super) fn flow_width(&self) -> Option<f32> {
        self.constraints.key_widths().1
    }
}
