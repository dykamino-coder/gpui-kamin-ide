//! Detached replacements let normalization and admission finish before publication.
use super::{CascadeEntry, NodeKey};
use crate::budget::admission::Error;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default)]
pub struct MemoUpdate {
    pub(super) entries: HashMap<NodeKey, Rc<CascadeEntry>>,
}

impl MemoUpdate {
    pub(super) fn get(&self, node: NodeKey) -> Option<&Rc<CascadeEntry>> {
        self.entries.get(&node)
    }

    pub(super) fn merge(&mut self, update: Self) -> Result<(), Error> {
        self.entries
            .try_reserve(update.entries.len())
            .map_err(|_| Error::Allocation)?;
        self.entries.extend(update.entries);
        Ok(())
    }

    pub fn replace(&mut self, entry: Rc<CascadeEntry>) -> Result<(), Error> {
        self.entries.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.entries.insert(entry.node, entry);
        Ok(())
    }
}
