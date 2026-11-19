//! Запись кадра документа в состоянии элемента окна (`element::Retained`).

use super::window_key::WindowKey;
use super::{FrameKey, Slot};
use gpui::{AnyWindowHandle, App, Bounds, Pixels, RetainedPaint, RetainedPrepaint, Window};
use std::{cell::Cell, rc::Rc};

/// Inputs of the actual recording. The element state owns these inputs. The document
/// can inspect them between frames without keeping an evicted element state alive.
pub(super) struct Inputs {
    pub(super) key: FrameKey,
    window: AnyWindowHandle,
    serial: Cell<u64>,
    environment: WindowKey,
}

impl Inputs {
    pub(super) fn new(key: FrameKey, environment: WindowKey, window: &Window) -> Rc<Self> {
        Rc::new(Self {
            key,
            window: window.window_handle(),
            serial: Cell::new(window.frame_serial()),
            environment,
        })
    }

    pub(super) fn matches(&self, bounds: Bounds<Pixels>, window: &Window, cx: &App) -> bool {
        self.window == window.window_handle()
            && self.serial.get().checked_add(1) == Some(window.frame_serial())
            && window.retained_root_environment()
            && self.environment == WindowKey::capture(bounds, window, cx)
    }

    pub(super) fn replayed(&self, window: &Window) {
        self.serial.set(window.frame_serial());
    }
}

#[derive(Default)]
pub(super) struct Record {
    pub(super) inputs: Option<Rc<Inputs>>,
    pub(super) prepaint: RetainedPrepaint,
    pub(super) paint: RetainedPaint,
    pub(super) root: bool,
}

impl Record {
    pub(super) fn matches(
        &self, slot: &Slot, key: Option<&FrameKey>, bounds: Bounds<Pixels>, window: &Window, cx: &App,
    ) -> bool {
        self.inputs.as_ref().zip(slot.inputs(key)).is_some_and(|(own, published)| {
            Rc::ptr_eq(own, &published) && own.matches(bounds, window, cx)
        })
    }

    pub(super) fn publish(&self, slot: &Slot, settled: bool) {
        let inputs = self.inputs.as_ref().filter(|_| settled).map(Rc::downgrade);
        slot.recorded.replace(inputs.unwrap_or_default());
    }
}

/// Both the rendering step and the element inspect the same recording.
impl Slot {
    pub(super) fn inputs(&self, key: Option<&FrameKey>) -> Option<Rc<Inputs>> {
        let inputs = self.recorded.borrow().upgrade()?;
        (Some(&inputs.key) == key).then_some(inputs)
    }
}
