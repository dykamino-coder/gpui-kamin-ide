//! Retained cascade output before document normalization, keyed by source identity.
//!
//! A stage owns replacements, never a copy or a predecessor of the committed map.
//! Entries contain detached matching inputs, not source handles or value owners.
use super::{Children, NodeKey};
use crate::budget::admission::{Admission, Error};
use crate::style::generated::counters::Counters;
use std::collections::HashMap;
use std::rc::Rc;

mod context;
mod dirty;
mod ownership;
mod publication;
mod readers;
mod retirement;
mod root;
mod stage;
mod update;
pub use context::{CascadeContext, EntryInputs, GeneratedContext, GroupInput};
pub use dirty::DirtyEntries;
pub use ownership::OutputOwnership;
pub use root::RootOutput;
pub use stage::MemoStage;
pub use update::MemoUpdate;

/// One source element can emit zero, one or several sibling boxes.
pub struct CascadeEntry {
    pub node: NodeKey,
    pub source_parent: Option<NodeKey>,
    pub ownership: OutputOwnership,
    pub inputs: EntryInputs,
    pub raw: Children,
    pub exit: Counters,
    /// Selection of an enclosing scroll-target group, replayed on the current walk.
    pub selects_group: Option<NodeKey>,
    _lease: Option<crate::budget::LiveReservation>,
}

impl CascadeEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node: NodeKey,
        source_parent: Option<NodeKey>,
        ownership: OutputOwnership,
        inputs: EntryInputs,
        raw: Children,
        exit: Counters,
        selects_group: Option<NodeKey>,
        admission: &Admission,
    ) -> Result<Rc<Self>, Error> {
        let bytes = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of_val(raw.as_slice()))
            .and_then(|n| n.checked_add(std::mem::size_of_val(ownership.children.as_slice())))
            .ok_or(Error::CapacityOverflow)?;
        let lease = admission.storage(bytes)?;
        Ok(Rc::new(Self {
            node,
            source_parent,
            ownership,
            inputs,
            raw,
            exit,
            selects_group,
            _lease: lease,
        }))
    }
}

/// Only the current revision's index is retained. Old frames own their actual nodes.
#[derive(Default)]
pub struct SourceMemo {
    entries: HashMap<NodeKey, Rc<CascadeEntry>>,
    readers: readers::Readers,
    output: RootOutput,
}

impl SourceMemo {
    pub fn get(&self, node: NodeKey) -> Option<&Rc<CascadeEntry>> {
        self.entries.get(&node)
    }

    pub fn roots(&self) -> &Children {
        &self.output.raw
    }

    pub fn output(&self) -> &RootOutput {
        &self.output
    }

    pub fn readers(&self, sequence: NodeKey) -> &[NodeKey] {
        self.readers.get(sequence)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stage<'a>(&'a mut self, delta: &super::source_delta::SourceDelta) -> MemoStage<'a> {
        MemoStage::new(self, delta)
    }
}

#[cfg(test)]
mod tests;
