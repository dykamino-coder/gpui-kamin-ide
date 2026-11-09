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
        let record = self.reuse.store.far_record(e.node_id)?;
        if !record.fits(&self.key, Counters::read(&opts.cx), e) {
            return None;
        }
        record.end.write(&opts.cx);
        let slot = Rc::new(FarSlot::new(record.clone()));
        self.reuse.with_far(|far| {
            far.next.insert(e.node_id, record);
            far.slots.push(slot.clone());
        });
        Some(Far::new(slot).hooked())
    }

    /// Wrap the built element of child `e` for a far record when its subtree
    /// may be claimed whole at another width (`far::far_replayable`).
    pub(super) fn record_far(
        &self,
        e: &Element,
        mark: &Mark,
        end: Counters,
        out: &mut Vec<AnyElement>,
    ) {
        // A build that read an outside width gives no far record.
        let Some(widths) = mark.widths.clone().filter(|w| mark.far && !w.get()) else {
            return;
        };
        let Some(child) = out.pop() else { return };
        let pending = Rc::new(PendingFar::new(self.key.clone(), mark.start, end, widths));
        let source = self.reuse.shells.source(e);
        self.reuse
            .with_far(|far| far.pending.push((pending.clone(), source)));
        out.push(FarRecorder::new(child, pending).hooked());
    }
}
