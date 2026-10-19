//! Records the laid-out root edge for this document's canvas painting.

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};
use std::rc::Rc;

/// Обёртка, которая на подготовке записывает левый край своей коробки минус
/// `offset` (поля тела и рамка/отбивка корня) в буфер документа — раскладку не
/// меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    measures: Rc<super::VerticalMeasures>,
    pub(crate) child: AnyElement,
    pub(crate) key: u64,
    pub(crate) offset: f32,
}

pub fn record_root_left(
    child: AnyElement,
    key: u64,
    offset: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> AnyElement {
    RecordRootLeft {
        child,
        key,
        offset,
        measures: vcx.doc.vertical_measures.clone(),
    }
    .hooked()
}

impl Wrap for RecordRootLeft {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let left = f32::from(bounds.origin.x) - self.offset;
        self.measures.root_left.borrow_mut().insert(self.key, left);
        self.child.prepaint(window, cx);
    }
}
