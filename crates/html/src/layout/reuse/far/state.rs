//! The far state of a document: its far records and the scratch of the
//! build that uses them, one allocation kept between frames (`Store::far`).

use super::{FarRecord, FarSlot};
use crate::layout::reuse::FlowReuse;
use gpui::InkSink;
use std::cell::Cell;
use std::rc::Rc;

/// Far records of a document and the far blocks of the build that holds
/// them (`FlowReuse::far`), allocated with the first far block. Between
/// builds the document keeps it while it keeps records (`Store::far`), so a
/// build that claims them allocates no far state of its own.
#[derive(Default)]
pub(crate) struct FrameFar {
    /// Records kept from earlier builds, sorted by node id.
    pub(in crate::layout::reuse) records: Vec<FarRecord>,
    /// Records claimed by this build.
    pub(in crate::layout::reuse) claimed: u64,
    /// Recorders this build made (`FarRecorder`): at most that many records.
    pub(in crate::layout::reuse) recorders: usize,
    /// Records this build's recorders made, until the build is finished.
    pub(in crate::layout::reuse) done: Vec<FarRecord>,
    pub(in crate::layout::reuse) slots: Vec<Rc<FarSlot>>,
    /// Outside width reads of the far candidate being built, shared by all
    /// candidates of the build (`Mark::shared_widths`).
    pub(in crate::layout::reuse) widths: Rc<Cell<bool>>,

    /// The ink sink of the recorders' paints, one scope per recorder
    /// (`InkSink::begin_scope`).
    pub(in crate::layout::reuse) sink: Rc<InkSink>,
}

impl FrameFar {
    /// Drop what belongs to the build, keep the records and the storage.
    fn rest(&mut self) {
        self.claimed = 0;
        self.recorders = 0;
        self.done.clear();
        self.slots.clear();
        self.sink.clear();
    }

    fn find(&self, node: u64) -> Option<&FarRecord> {
        let at = self
            .records
            .binary_search_by_key(&node, FarRecord::id)
            .ok()?;
        self.records.get(at)
    }
}

impl FlowReuse {
    /// This build's far state: the document's, or made with its first use.
    pub(in crate::layout::reuse) fn with_far<R>(&self, f: impl FnOnce(&mut FrameFar) -> R) -> R {
        let mut far = self
            .far
            .take()
            .or_else(|| self.store.far.take())
            .unwrap_or_default();
        let out = f(&mut far);
        self.far.set(Some(far));
        out
    }

    /// `f` of the far record of block `node`, if any; makes no far state.
    pub(in crate::layout::reuse) fn with_far_record<R>(
        &self,
        node: u64,
        f: impl FnOnce(&FarRecord) -> R,
    ) -> Option<R> {
        let far = self.far.take().or_else(|| self.store.far.take())?;
        let out = far.find(node).map(f);
        self.far.set(Some(far));
        out
    }

    /// The recorders' ink sink of this build, while it is not finished (a
    /// later paint of the same tree finds none).
    pub(in crate::layout::reuse) fn far_sink(&self) -> Option<Rc<InkSink>> {
        let far = self.far.take()?;
        let sink = far.sink.clone();
        self.far.set(Some(far));
        Some(sink)
    }

    /// A recorder of this build made `record` (`FarRecorder::paint`). The
    /// first one reserves room for all of the build's recorders.
    pub(in crate::layout::reuse) fn add_far(&self, record: FarRecord) {
        self.with_far(|far| {
            if far.done.is_empty() {
                far.done.reserve_exact(far.recorders);
            }
            far.done.push(record);
        });
    }

    /// After paint: far records claimed in this frame and those recorded.
    /// A failed recording marks its block near (`Store::far_near`).
    pub(in crate::layout::reuse) fn finish_far(&self) {
        let Some(mut far) = self.far.take() else {
            // No far block in this build: no record is carried over.
            self.store.far.take();
            return;
        };
        let claimed = far.claimed;
        // The records claimed by this build stay; a build that claimed none
        // (the second build of a refused frame) keeps none, like the marks
        // of a refused build it replaces.
        far.records
            .retain(|record| record.claimed.replace(false) && claimed > 0);
        let mut done = std::mem::take(&mut far.done);
        done.retain(|record| {
            let kept = !far.sink.scope_tainted(record.scope);
            match kept {
                true => self.store.forget_near(record.id()),
                false => self.store.note_near(record.id()),
            }
            kept
        });
        if !done.is_empty() {
            // The recording build's list becomes the records: no copy.
            match far.records.is_empty() {
                true => far.records = done,
                false => {
                    far.records.append(&mut done);
                    far.done = done;
                }
            }
            far.records.sort_unstable_by_key(FarRecord::id);
        }
        far.rest();
        self.store.far.set((!far.records.is_empty()).then_some(far));
        let mut stats = self.store.stats.get();
        stats.far += claimed;
        self.store.stats.set(stats);
    }
}

impl Drop for FlowReuse {
    /// A build dropped unfinished (refused and built again) leaves the
    /// document's records as they were.
    fn drop(&mut self) {
        if let Some(mut far) = self.far.take() {
            far.rest();
            let kept = self.store.far.take();
            self.store
                .far
                .set(kept.or((!far.records.is_empty()).then_some(far)));
        }
    }
}
