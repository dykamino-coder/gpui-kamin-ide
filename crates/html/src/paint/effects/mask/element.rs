//! Элементы MaskKeep и MaskUse: снимок и применение маски.

use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, Pixels, Window};

/// Маска обрезающего предка для ОТЛОЖЕННОГО слоя (`z-index > 0`).
///
/// GPUI рисует отложенные элементы после всего дерева и восстанавливает им
/// стек id и стилей, но не маску содержимого: `overflow: hidden/clip`
/// предка их не режет (`overflow-clip-margin-011..022`). Наружная обёртка
/// стоит в потоке и на `prepaint` запоминает текущую маску, внутренняя —
/// внутри `deferred` — рисует ребёнка под ней. Фиксированному слою маска
/// не нужна: его содержащий блок — окно.
pub type MaskCell = std::rc::Rc<std::cell::RefCell<Option<gpui::ContentMask<Pixels>>>>;

pub struct MaskKeep {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl Wrap for MaskKeep {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        *self.cell.borrow_mut() = Some(window.content_mask());
        self.child.prepaint(window, cx);
    }
}

pub struct MaskUse {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl Wrap for MaskUse {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let mask = *self.cell.borrow();
        window.with_content_mask(mask, |window| self.child.prepaint(window, cx));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let mask = *self.cell.borrow();
        window.with_content_mask(mask, |window| self.child.paint(window, cx));
    }
}
