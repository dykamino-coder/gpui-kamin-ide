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
        let start = Counters::read(&opts.cx);
        let slot = self.reuse.with_far_record(e.node_id, |at, record| {
            if !record.fits(start, e) {
                return None;
            }
            record.end.write(&opts.cx);
            record.claim();
            Some(Rc::new(FarSlot::new(at, record, !self.settled)))
        })??;
        self.reuse.with_far(|far| {
            far.claimed += 1;
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
        // A build that read an outside width gives no far record. Widths are
        // read during the build only, so the flag is final here.
        if !mark.far || !mark.shared_widths || self.reuse.far_widths.get() {
            return;
        }
        let Some(child) = out.pop() else { return };
        let pending = PendingFar::new(mark.start, end);
        let source = self.reuse.shells.source(e);
        self.reuse.add_far_recorder();
        out.push(FarRecorder::new(child, self.reuse.clone(), pending, source).hooked());
    }
}
