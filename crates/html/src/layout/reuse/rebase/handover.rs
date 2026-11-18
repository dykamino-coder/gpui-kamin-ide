//! The store around a rebuild or a patch of its document: what is released
//! before the rebuild, and what is left when nothing recorded holds.

use super::Store;

impl Store {
    /// The document was patched (`Document::commit`): records compared at a
    /// rebuild (`rebase`) no longer hold, the others are compared by the
    /// next build.
    pub(crate) fn patch(&self) {
        self.patched.set(true);
        let mut records = self.records.take();
        records.retain(|_, r| r.source.holds());
        self.records.replace(records);
    }

    /// The document `old` was restyled in place as `new` (copied paths, the
    /// same node ids), the elements `restyled` with new cascaded values: the
    /// records are compared as at a rebuild (`rebase`), so a run of such
    /// restyles keeps the far blocks and stops recording the near ones, as a
    /// run of rebuilds does. After a patch since the last frame they hold only
    /// by the next build's comparison (`patch`).
    pub(crate) fn restyle(
        &self,
        old: &[crate::dom::Node],
        new: &[crate::dom::Node],
        restyled: &[u64],
    ) {
        if self.patched.get() {
            self.patch();
            return;
        }
        for record in self.records.borrow().values() {
            record.source.release();
        }
        self.rebase(old, new, Some(restyled));
    }

    /// The document is about to be rebuilt from the same source DOM, and the
    /// rebuild hands this store over (`rebase`, which compares document boxes,
    /// not record sources): the records release their sources, blocks of the
    /// previous frame tree, before the new tree is built, so the previous frame
    /// tree does not live beside it (Blink keeps a layout result, not the
    /// inputs that made it). After a patch the next build compares by the
    /// sources, so they stay.
    pub(crate) fn release_sources(&self) {
        if self.patched.get() {
            return;
        }
        for record in self.records.borrow().values() {
            record.source.release();
        }
    }

    /// The rebuild of `release_sources` failed: the records cannot be
    /// compared with the document any more.
    pub(crate) fn forget(&self) {
        if !self.patched.get() {
            self.start_over();
        }
    }

    /// Nothing recorded holds (`rebase`): the state of a fresh document, so
    /// the next build is not settled and records nothing, as a fresh one. A
    /// run of restyles (`restyled`) goes on: a block whose record a restyle
    /// dropped is not recorded again while every frame follows a rebuild.
    pub(in crate::layout::reuse) fn start_over(&self) {
        self.reset();
        self.reset_replays();
        self.unrecorded.take();
        self.refused.take();
        self.last.take();
        self.prev.take();
        self.settled.set(false);
        self.warm.set(false);
    }

    /// The last frame wrapped blocks and none gave a record.
    pub(super) fn tried(&self) -> bool {
        let unrecorded = self.unrecorded.take();
        let tried = !unrecorded.is_empty();
        self.unrecorded.set(unrecorded);
        tried
    }
}
