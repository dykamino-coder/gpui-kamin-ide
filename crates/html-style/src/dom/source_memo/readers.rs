//! Reverse sequence dependencies are updated atomically with the current entries.
use super::{CascadeEntry, NodeKey};
use crate::budget::admission::Error;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Readers(HashMap<NodeKey, Vec<NodeKey>>);

impl Readers {
    pub fn get(&self, sequence: NodeKey) -> &[NodeKey] {
        self.0.get(&sequence).map_or(&[], Vec::as_slice)
    }

    pub fn prepare(
        &mut self,
        entries: &HashMap<NodeKey, Rc<CascadeEntry>>,
        replacements: &HashMap<NodeKey, Rc<CascadeEntry>>,
        retired: &HashSet<NodeKey>,
    ) -> Result<HashMap<NodeKey, Vec<NodeKey>>, Error> {
        let mut edits = HashMap::new();
        for node in retired.iter().chain(replacements.keys()) {
            if let Some(sequence) = entries.get(node).and_then(|e| e.ownership.sequence) {
                self.edit(&mut edits, sequence)?
                    .retain(|owner| owner != node);
            }
        }
        for entry in replacements.values() {
            if let Some(sequence) = entry.ownership.sequence {
                let readers = self.edit(&mut edits, sequence)?;
                readers.try_reserve(1).map_err(|_| Error::Allocation)?;
                readers.push(entry.node);
            }
        }
        self.0
            .try_reserve(edits.len())
            .map_err(|_| Error::Allocation)?;
        Ok(edits)
    }

    fn edit<'a>(
        &self,
        edits: &'a mut HashMap<NodeKey, Vec<NodeKey>>,
        sequence: NodeKey,
    ) -> Result<&'a mut Vec<NodeKey>, Error> {
        if !edits.contains_key(&sequence) {
            let old = self.get(sequence);
            let mut copied = Vec::new();
            copied
                .try_reserve_exact(old.len())
                .map_err(|_| Error::Allocation)?;
            copied.extend_from_slice(old);
            edits.try_reserve(1).map_err(|_| Error::Allocation)?;
            edits.insert(sequence, copied);
        }
        Ok(edits.get_mut(&sequence).expect("prepared readers"))
    }

    pub fn publish(&mut self, edits: HashMap<NodeKey, Vec<NodeKey>>) {
        for (sequence, readers) in edits {
            if readers.is_empty() {
                self.0.remove(&sequence);
            } else {
                self.0.insert(sequence, readers);
            }
        }
    }
}
