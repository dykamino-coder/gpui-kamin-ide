//! A full cascade and a dirty walk publish through the same ownership transaction.
use super::{MemoUpdate, NodeKey, RootOutput, SourceMemo};
use crate::budget::admission::Error;
use std::collections::{HashMap, HashSet};

/// Holds the committed revision exclusively until all consumers can publish.
pub struct PreparedMemo<'a> {
    base: &'a mut SourceMemo,
    update: MemoUpdate,
    output: RootOutput,
    retired: HashSet<NodeKey>,
    readers: HashMap<NodeKey, Vec<NodeKey>>,
}

impl PreparedMemo<'_> {
    /// The single retirement set shared by cascade and binding publication.
    pub fn retired(&self) -> &HashSet<NodeKey> {
        &self.retired
    }

    /// Preparation has reserved all map capacity; dropping instead aborts.
    pub fn publish(self) {
        for node in self.retired {
            self.base.entries.remove(&node);
        }
        self.base.entries.extend(self.update.entries);
        self.base.readers.publish(self.readers);
        self.base.output = self.output;
    }
}

impl SourceMemo {
    /// All fallible preparation precedes changes to committed entries and roots.
    pub fn prepare(
        &mut self,
        update: MemoUpdate,
        output: RootOutput,
    ) -> Result<PreparedMemo<'_>, Error> {
        let retired = super::retirement::removed(self, &update.entries, &output)?;
        let readers = self
            .readers
            .prepare(&self.entries, &update.entries, &retired)?;
        self.entries
            .try_reserve(update.entries.len())
            .map_err(|_| Error::Allocation)?;
        Ok(PreparedMemo {
            base: self,
            update,
            output,
            retired,
            readers,
        })
    }

    pub fn publish(&mut self, update: MemoUpdate, output: RootOutput) -> Result<(), Error> {
        self.prepare(update, output)?.publish();
        Ok(())
    }
}
