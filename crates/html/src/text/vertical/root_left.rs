//! Records the left edge of the root's content-sized box under `vertical-rl`
//! for the canvas background (`DocState::root_left`).

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};

/// Обёртка тела, которая на подготовке записывает левый край своей коробки
/// минус `offset` (поля тела и рамка/отбивка корня) в состояние документа —
/// раскладку не меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    pub(crate) child: AnyElement,
    pub(crate) offset: f32,
    pub(crate) values: crate::document::state::ValueDoc,
}

pub fn record_root_left(
    child: AnyElement,
    offset: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> AnyElement {
    RecordRootLeft {
        child,
        offset,
        values: vcx.keep(),
    }
    .hooked()
}

impl Wrap for RecordRootLeft {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let left = f32::from(bounds.origin.x) - self.offset;
        self.values.vcx().doc.root_left.set(Some(left));
        self.child.prepaint(window, cx);
    }
}
