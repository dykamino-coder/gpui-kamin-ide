//! A full cascade and a dirty walk publish through the same ownership transaction.
use super::{MemoUpdate, RootOutput, SourceMemo};
use crate::budget::admission::Error;

impl SourceMemo {
    /// All fallible preparation precedes changes to committed entries and roots.
    pub fn publish(&mut self, update: MemoUpdate, output: RootOutput) -> Result<(), Error> {
        let retired = super::retirement::removed(self, &update.entries, &output)?;
        let readers = self
            .readers
            .prepare(&self.entries, &update.entries, &retired)?;
        self.entries
            .try_reserve(update.entries.len())
            .map_err(|_| Error::Allocation)?;
        for node in retired {
            self.entries.remove(&node);
        }
        self.entries.extend(update.entries);
        self.readers.publish(readers);
        self.output = output;
        Ok(())
    }
}
