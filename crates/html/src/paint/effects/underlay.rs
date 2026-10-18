//! Элемент `Underlay`.
// owner: A

use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, Pixels, Window};

/// Подложка: поддерево рисуется ПОД всем содержимым кадра.
///
/// Отрицательный `z-index` (CSS 2.1 §9.9, шаг 3): элемент остаётся на своём
/// месте в потоке — его слот и есть его координата, — но краска ложится ниже
/// содержимого, нарисованного до него. Порядок отрисовки у нас — порядок
/// детей, и позднему ребёнку иначе никак не лечь под раннего.
pub struct Underlay {
    pub(crate) child: AnyElement,
}

impl Underlay {
    pub fn new(child: AnyElement) -> Self {
        Underlay { child }
    }
}

impl Wrap for Underlay {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.paint_bottom_layer(|window| self.child.paint(window, cx));
    }
}
