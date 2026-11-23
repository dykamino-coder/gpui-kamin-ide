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

    pub fn replace(&mut self, entry: Rc<CascadeEntry>) -> Result<(), Error> {
        self.entries.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.entries.insert(entry.node, entry);
        Ok(())
    }
}
