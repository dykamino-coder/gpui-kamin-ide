//! Запись кадра документа в состоянии элемента окна (`element::Retained`).

use super::window_key::WindowKey;
use super::{FrameKey, Slot};
use gpui::{AnyWindowHandle, RetainedPaint, RetainedPrepaint, Window};
use std::{cell::Cell, rc::Rc};

/// Inputs of the actual recording. The element state owns these inputs. The document
/// can inspect them between frames without keeping an evicted element state alive.
pub(super) struct Inputs {
    pub(super) key: FrameKey,
    window: AnyWindowHandle,
    serial: Cell<u64>,
    environment: WindowKey,
    pub(super) layout: gpui::RetainedLayoutInputs,
}

impl Inputs {
    pub(super) fn new(
        key: FrameKey,
        environment: WindowKey,
        window: &Window,
        layout: gpui::RetainedLayoutInputs,
    ) -> Rc<Self> {
        Rc::new(Self {
            key,
            window: window.window_handle(),
            serial: Cell::new(window.frame_serial()),
            environment,
            layout,
        })
    }

    pub(super) fn matches(&self, environment: &WindowKey, window: &Window) -> bool {
        self.window == window.window_handle()
            && self.serial.get().checked_add(1) == Some(window.frame_serial())
            && window.retained_root_environment()
            && self.environment == *environment
            && self.layout.scroll_unchanged()
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
    /// Window frame and document generation that own this element state.
    frame: Option<(u64, u64)>,
    painted: bool,
}

impl Record {
    /// The rendering step and the actual draw enter the same generation.
    /// Only a painted predecessor in this window can supply replay ranges.
    pub(super) fn begin(&mut self, store: &crate::layout::reuse::Store, window: &Window) -> u64 {
        let serial = window.frame_serial();
        let epoch = store.epoch();
        if self.frame == Some((serial, epoch)) {
            return epoch;
        }
        let continued = self.painted
            && self.frame.is_some_and(|(prior, generation)| {
                generation == epoch && prior.checked_add(1) == Some(serial)
            });
        let next = store.draw(continued);
        self.frame = Some((serial, next));
        self.painted = false;
        next
    }

    pub(super) fn painted(&mut self) {
        self.painted = true;
    }

    pub(super) fn matches(
        &self,
        slot: &Slot,
        key: Option<&FrameKey>,
        environment: Option<&WindowKey>,
        window: &Window,
    ) -> bool {
        self.inputs
            .as_ref()
            .zip(slot.inputs(key))
            .is_some_and(|(own, published)| {
                Rc::ptr_eq(own, &published)
                    && environment.is_some_and(|env| own.matches(env, window))
            })
    }

    pub(super) fn publish(&self, slot: &Slot, settled: bool) {
        let inputs = self
            .inputs
            .as_ref()
            .filter(|inputs| settled && inputs.layout.scroll_unchanged())
            .map(Rc::downgrade);
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
