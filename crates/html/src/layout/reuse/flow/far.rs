//! A far child of the root flow laid out from its kept layout subtree
//! (`far`): the record lookup before a build and the wrapper after it.

use super::super::far::{Far, FarRecorder, FarSlot, PendingFar};
use super::{Counters, FlowCx, Mark};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;
use std::rc::Rc;

impl FlowCx {
    /// Stand-in of child `e` laid out from its far record, if it fits. The
    /// caller (`replay`) has checked what a subtree claim needs: no repeated
    /// build of the frame, no float before, no ban or refusal.
    pub(super) fn far(&self, e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
        let record = self.reuse.store.far.borrow().get(&e.node_id).cloned()?;
        if !record.fits(&self.key, Counters::read(&opts.cx), e) {
            return None;
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next_far.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next_far.replace(next);
        let slot = Rc::new(FarSlot::new(record));
        super::super::push(&self.reuse.far_slots, slot.clone());
        Some(Far::new(slot).hooked())
    }

    /// Wrap the built element of child `e` for a far record when its subtree
    /// may be claimed whole at another width (`plain::far_replayable`).
    pub(super) fn record_far(
        &self,
        e: &Element,
        mark: &Mark,
        end: Counters,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let Some(widths) = mark.widths.clone() else {
            return;
        };
        if !self.far_worth_recording(e) || !super::super::plain::far_replayable(e, opts.cx.vcx()) {
            return;
        }
        let Some(child) = out.pop() else { return };
        let pending = Rc::new(PendingFar::new(self.key.clone(), mark.start, end, widths));
        let source = self.reuse.shells.source(e);
        super::super::push(&self.reuse.pending_far, (pending.clone(), source));
        out.push(FarRecorder::new(child, pending).hooked());
    }
}
