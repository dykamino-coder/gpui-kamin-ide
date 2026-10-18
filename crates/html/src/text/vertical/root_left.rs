//! Root left for vertical; split out to keep the owning module within 250 lines.

use super::ROOT_LEFT;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};

/// Левый край коробки корня (см. `ROOT_LEFT`).
pub fn root_left_prev(key: u64) -> Option<f32> {
    ROOT_LEFT.with(|c| c.borrow().get(&key).copied())
}

/// Обёртка, которая на подготовке записывает левый край своей коробки минус
/// `offset` (поля тела и рамка/отбивка корня) в `ROOT_LEFT` — раскладку не
/// меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    pub(crate) child: AnyElement,
    pub(crate) key: u64,
    pub(crate) offset: f32,
}

pub fn record_root_left(child: AnyElement, key: u64, offset: f32) -> AnyElement {
    RecordRootLeft { child, key, offset }.hooked()
}

impl Wrap for RecordRootLeft {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let left = f32::from(bounds.origin.x) - self.offset;
        ROOT_LEFT.with(|c| c.borrow_mut().insert(self.key, left));
        self.child.prepaint(window, cx);
    }
}
