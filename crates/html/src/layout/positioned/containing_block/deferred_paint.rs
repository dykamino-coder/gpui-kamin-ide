//! Публикация построенного у CB элемента для отложенной покраски в исходном месте дерева.

use gpui::{AnyElement, ContentMask, Pixels};
use std::{cell::RefCell, rc::Rc};

type DeferredChild = (AnyElement, ContentMask<Pixels>);

#[derive(Clone, Default)]
pub(crate) struct DeferredPaint(Rc<RefCell<Option<DeferredChild>>>);

impl DeferredPaint {
    pub(crate) fn publish(&self, child: AnyElement, mask: ContentMask<Pixels>) {
        *self.0.borrow_mut() = Some((child, mask));
    }

    /// Доступ к текущему элементу с удержанием заимствования общего слота.
    pub(crate) fn with_child(&self, paint: impl FnOnce(&mut AnyElement, ContentMask<Pixels>)) {
        if let Some((child, mask)) = self.0.borrow_mut().as_mut() {
            paint(child, *mask);
        }
    }
}
