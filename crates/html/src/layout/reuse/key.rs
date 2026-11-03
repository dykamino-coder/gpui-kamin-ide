//! Ключ корневого потока и счётчики сборки, которые продвигает его ребёнок.

use crate::dom::Element;
use crate::layout::cx::{BoxConstraints, LayoutCx};

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

/// Окружение корневого потока, от которого зависит раскладка его детей.
pub(crate) struct FlowKey {
    pub(super) html: Element,
    pub(super) body: Element,
    pub(super) constraints: BoxConstraints,
    pub(super) viewport: (f32, f32),
    pub(super) text: gpui::TextStyle,
    pub(super) normal_line_height: f32,
    pub(super) fonts: u64,
}

impl FlowKey {
    pub(super) fn same(&self, other: &Self) -> bool {
        crate::dom::same_box(&self.html, &other.html)
            && crate::dom::same_box(&self.body, &other.body)
            && self.constraints == other.constraints
            && self.viewport == other.viewport
            && self.text == other.text
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }
}
