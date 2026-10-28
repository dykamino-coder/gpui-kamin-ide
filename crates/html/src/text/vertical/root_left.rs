//! Левый край коробки корня по содержимому при `vertical-rl` для фона холста
//! (`LayoutState::root_left`): обёртка `RecordRootLeft` на подготовке пишет левый край своей
//! коробки минус поля тела и рамку корня, не меняя раскладки; отрисовка холста того же кадра
//! позиционирует от него фон (CSS 2.2 §14.2).

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};

/// Обёртка тела, которая на подготовке записывает левый край своей коробки
/// минус `offset` (поля тела и рамка/отбивка корня) в состояние документа —
/// раскладку не меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    pub(crate) child: AnyElement,
    pub(crate) offset: f32,
    pub(crate) layout: std::rc::Rc<crate::layout::layout_state::LayoutState>,
}

pub fn record_root_left(
    child: AnyElement,
    offset: f32,
    layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
) -> AnyElement {
    RecordRootLeft {
        child,
        offset,
        layout: layout.clone(),
    }
    .hooked()
}

impl Wrap for RecordRootLeft {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let left = f32::from(bounds.origin.x) - self.offset;
        self.layout.root_left.set(Some(left));
        self.child.prepaint(window, cx);
    }
}
