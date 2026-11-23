//! An aborted cascade drops staged entries; publication touches only changed keys.
use super::{
    CascadeEntry, DirtyEntries, EntryInputs, MemoUpdate, NodeKey, PreparedMemo, RootOutput,
    SourceMemo,
};
use crate::budget::admission::Error;
use crate::dom::source_delta::SourceDelta;
use std::rc::Rc;

pub struct MemoStage<'a> {
    base: &'a mut SourceMemo,
    pub(super) update: MemoUpdate,
    dirty: DirtyEntries,
}

impl<'a> MemoStage<'a> {
    pub(super) fn new(base: &'a mut SourceMemo, delta: &SourceDelta) -> Self {
        let dirty = DirtyEntries::new(base, delta);
        Self {
            base,
            update: MemoUpdate::default(),
            dirty,
        }
    }

    pub fn dirty(&self) -> &DirtyEntries {
        &self.dirty
    }

    pub fn invalidate(&mut self, node: NodeKey) {
        self.dirty.invalidate(self.base, node);
    }

    pub fn get(&self, node: NodeKey) -> Option<&Rc<CascadeEntry>> {
        self.update.get(node).or_else(|| self.base.get(node))
    }

    pub fn reusable(&self, node: NodeKey, inputs: &EntryInputs) -> Option<&Rc<CascadeEntry>> {
        self.get(node)
            .filter(|entry| !self.dirty.contains(node) && entry.inputs == *inputs)
    }

    pub fn replace(&mut self, entry: Rc<CascadeEntry>) -> Result<(), Error> {
        self.update.replace(entry)
    }

    /// Reserve before touching the committed map. Allocation failure is an abort.
    pub fn prepare(self, output: RootOutput) -> Result<PreparedMemo<'a>, Error> {
        self.base.prepare(self.update, output)
    }

    pub fn commit(self, output: RootOutput) -> Result<(), Error> {
        self.prepare(output)?.publish();
        Ok(())
    }
}
