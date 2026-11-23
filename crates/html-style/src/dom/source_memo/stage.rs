//! An aborted cascade drops staged entries; publication touches only changed keys.
use super::{CascadeEntry, DirtyEntries, EntryInputs, NodeKey, RootOutput, SourceMemo};
use crate::budget::admission::Error;
use crate::dom::source_delta::SourceDelta;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct MemoStage<'a> {
    base: &'a mut SourceMemo,
    replacements: HashMap<NodeKey, Rc<CascadeEntry>>,
    retired: HashSet<NodeKey>,
    dirty: DirtyEntries,
}

impl<'a> MemoStage<'a> {
    pub(super) fn new(base: &'a mut SourceMemo, delta: &SourceDelta) -> Self {
        let dirty = DirtyEntries::new(base, delta);
        Self {
            base,
            replacements: HashMap::new(),
            retired: HashSet::new(),
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
        if self.retired.contains(&node) {
            return None;
        }
        self.replacements.get(&node).or_else(|| self.base.get(node))
    }

    pub fn reusable(&self, node: NodeKey, inputs: &EntryInputs) -> Option<&Rc<CascadeEntry>> {
        self.get(node)
            .filter(|entry| !self.dirty.contains(node) && entry.inputs == *inputs)
    }

    pub fn replace(&mut self, entry: Rc<CascadeEntry>) -> Result<(), Error> {
        self.replacements
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        self.retired.remove(&entry.node);
        self.replacements.insert(entry.node, entry);
        Ok(())
    }

    /// Explicit retirement follows only removed output descendants. A caller must
    /// claim moved descendants first; their new entries survive this traversal.
    pub fn retire(&mut self, root: NodeKey) {
        let mut work = vec![root];
        while let Some(node) = work.pop() {
            if self.replacements.contains_key(&node) || !self.retired.insert(node) {
                continue;
            }
            if let Some(entry) = self.base.get(node) {
                work.extend_from_slice(&entry.ownership.children);
            }
        }
    }

    /// Reserve before touching the committed map. Allocation failure is an abort.
    pub fn commit(self, output: RootOutput) -> Result<(), Error> {
        let readers =
            self.base
                .readers
                .prepare(&self.base.entries, &self.replacements, &self.retired)?;
        self.base
            .entries
            .try_reserve(self.replacements.len())
            .map_err(|_| Error::Allocation)?;
        for node in self.retired {
            self.base.entries.remove(&node);
        }
        self.base.entries.extend(self.replacements);
        self.base.readers.publish(readers);
        self.base.output = output;
        Ok(())
    }
}
