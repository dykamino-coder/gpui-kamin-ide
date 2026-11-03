//! Запись кадра документа в состоянии элемента окна (`element::Retained`).

use super::window_key::WindowKey;
use super::{FrameKey, Slot};
use gpui::{RetainedPaint, RetainedPrepaint};
use std::rc::{Rc, Weak};

/// Запись кадра в состоянии элемента окна: живёт, только пока элемент
/// рисуется каждый кадр, поэтому индексы всегда указывают в прошлый кадр.
#[derive(Default)]
pub(super) struct Record {
    pub(super) slot: Weak<Slot>,
    pub(super) key: Option<FrameKey>,
    pub(super) window: Option<WindowKey>,
    pub(super) prepaint: RetainedPrepaint,
    pub(super) paint: RetainedPaint,
    /// Prepaint шёл в корневом окружении окна.
    pub(super) root: bool,
    pub(super) settled: bool,
}

impl Record {
    pub(super) fn reusable(
        &self,
        slot: &Rc<Slot>,
        key: &Option<FrameKey>,
        window: &WindowKey,
    ) -> bool {
        self.settled
            && self.paint.replayable()
            && self
                .slot
                .upgrade()
                .is_some_and(|own| Rc::ptr_eq(&own, slot))
            && key.is_some()
            && self.key == *key
            && self.window.as_ref() == Some(window)
    }
}
