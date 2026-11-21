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
    pub(super) layered: Option<RetainedPaint>,
    /// Layer whose unchanged commands still occupy the nested block ranges.
    pub(super) layer: Option<(u64, u64)>,
    pub(super) root: bool,
    /// Window frame and document generation that own this element state.
    frame: Option<(u64, u64)>,
    painted: bool,
}

mod miss;
mod replay;
pub(super) use miss::{FrameMiss, MISSES};
pub(super) use replay::Replay;

impl Record {
    pub(super) fn replay_paint(
        &mut self,
        window: &mut gpui::Window,
        store: &crate::layout::reuse::Store,
        same_prepaint: bool,
    ) {
        let paint = window.replay_retained_paint(&self.paint);
        if same_prepaint
            && paint.preserves_window_slots(&self.paint)
            && self.layer.is_some()
            && self.layer == store.layer_version()
        {
            store.copied_layer_ranges();
        }
        self.paint = paint;
    }

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

    /// Publish the record for replay, or keep why it is not (`refusal`).
    #[track_caller]
    pub(super) fn publish(&self, slot: &Slot, refusal: Option<FrameMiss>) {
        let read = self
            .inputs
            .as_ref()
            .is_some_and(|inputs| !inputs.layout.scroll_unchanged());
        let refusal = refusal.or(read.then_some(FrameMiss::ScrollRead));
        if let Some(refusal) = refusal {
            // The next frame misses for this reason; the journal names it now, at the publish.
            crate::why::note("frame_unpublished", || refusal.code().into());
        }
        let inputs = self
            .inputs
            .as_ref()
            .filter(|_| refusal.is_none())
            .map(Rc::downgrade);
        slot.unsettled.set(refusal);
        slot.recorded.replace(inputs.unwrap_or_default());
    }
}

/// Both the rendering step and the element inspect the same recording.
impl Slot {
    pub(super) fn inputs(&self, key: Option<&FrameKey>) -> Result<Rc<Inputs>, FrameMiss> {
        let unpublished = || self.unsettled.get().unwrap_or(FrameMiss::Unpublished).noted();
        let inputs = self.recorded.borrow().upgrade().ok_or_else(unpublished)?;
        (Some(&inputs.key) == key)
            .then_some(inputs)
            .ok_or_else(|| FrameMiss::Key.noted())
    }

    /// Count a build of the frame record for `miss`.
    pub(super) fn missed(&self, miss: FrameMiss) {
        miss.count(&self.misses);
    }

    /// The build reasons counted since the last call (name, count), non-zero only.
    pub(super) fn take_misses(&self) -> Vec<(String, u64)> {
        let counts = self.misses.take();
        (0..MISSES)
            .filter(|&at| counts[at] > 0)
            .map(|at| (FrameMiss::name(at), counts[at]))
            .collect()
    }
}
