//! Prepare every allocation and validation before changing committed bindings.
use super::*;
use crate::budget::admission::{Admission, Error};
use std::collections::HashSet;

pub struct BindingUpdate {
    pub(super) identity: Rc<()>,
    pub(super) generation: u64,
    pub(super) changed: HashMap<NodeKey, Rc<BindingSegment>>,
}

pub struct BindingStage<'a> {
    store: &'a BindingStore,
    changed: HashMap<NodeKey, Rc<BindingSegment>>,
}

impl<'a> BindingStage<'a> {
    pub(super) fn new(store: &'a BindingStore) -> Self {
        Self {
            store,
            changed: HashMap::new(),
        }
    }

    /// No descendant walk or binding-map copy. A failed admission changes nothing.
    pub fn resume(&self, segment: &Rc<BindingSegment>) -> bool {
        !self.changed.contains_key(&segment.owner)
            && self
                .store
                .segments
                .get(&segment.owner)
                .is_some_and(|current| Rc::ptr_eq(current, segment))
    }

    pub fn replace(&mut self, segment: Rc<BindingSegment>) -> Result<(), Error> {
        self.changed.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.changed.insert(segment.owner, segment);
        Ok(())
    }

    /// Merge direct bindings captured by the ordinary dirty walk. Its source
    /// identity belongs to the caller's already admitted current source DOM.
    pub fn recorded(&mut self, update: BindingRecording) -> Result<(), Error> {
        self.changed
            .try_reserve(update.changed.len())
            .map_err(|_| Error::Allocation)?;
        self.changed.extend(update.changed);
        Ok(())
    }

    pub fn into_update(self) -> BindingUpdate {
        BindingUpdate {
            identity: self.store.identity.clone(),
            generation: self.store.generation,
            changed: self.changed,
        }
    }
}

/// Owns only changes; dropping it leaves the committed store untouched.
pub struct BindingPublication<'a> {
    store: &'a mut BindingStore,
    update: BindingUpdate,
    retired: HashSet<NodeKey>,
    nodes: HashMap<NodeKey, Counts>,
    next_generation: u64,
}

impl BindingStore {
    /// `retired` is the result of the SAME ownership reconciliation as memo/boxes.
    /// A stale payload is rejected before any logical mutation. Capacity reserves
    /// may grow backing storage but never publish partial source bindings.
    pub fn prepare(
        &mut self,
        update: BindingUpdate,
        retired: impl IntoIterator<Item = NodeKey>,
        admission: &Admission,
    ) -> Result<Option<BindingPublication<'_>>, Error> {
        let current = Rc::ptr_eq(&update.identity, &self.identity);
        if !current || update.generation != self.generation {
            return Ok(None);
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(Error::CapacityOverflow)?;
        let mut removed = HashSet::new();
        for key in retired {
            removed.try_reserve(1).map_err(|_| Error::Allocation)?;
            removed.insert(key);
        }
        // Relocation claims belong to the memo's common retirement algorithm.
        if removed.iter().any(|k| update.changed.contains_key(k)) {
            return Ok(None);
        }
        let mut nodes = HashMap::new();
        for key in removed.iter().chain(update.changed.keys()) {
            if let Some(segment) = self.segments.get(key) {
                for &(key, value) in segment.direct.iter() {
                    admission.work(1)?;
                    nodes.try_reserve(1).map_err(|_| Error::Allocation)?;
                    let entry = nodes.entry(key).or_insert_with(|| self.nodes[&key]);
                    for (index, flag) in [ELEMENT, TEXT, READ].into_iter().enumerate() {
                        if value.flags & flag != 0 {
                            entry.flags[index] -= 1;
                        }
                    }
                }
            }
        }
        for segment in update.changed.values() {
            for &(key, value) in segment.direct.iter() {
                admission.work(1)?;
                nodes.try_reserve(1).map_err(|_| Error::Allocation)?;
                let entry = nodes
                    .entry(key)
                    .or_insert_with(|| self.nodes.get(&key).copied().unwrap_or_default());
                if entry.flags != [0; 3] && entry.address != value.address {
                    return Ok(None);
                }
                entry.address = value.address;
                for (index, flag) in [ELEMENT, TEXT, READ].into_iter().enumerate() {
                    if value.flags & flag != 0 {
                        entry.flags[index] = entry.flags[index]
                            .checked_add(1)
                            .ok_or(Error::CapacityOverflow)?;
                    }
                }
            }
        }
        self.segments
            .try_reserve(update.changed.len())
            .map_err(|_| Error::Allocation)?;
        self.nodes
            .try_reserve(nodes.len())
            .map_err(|_| Error::Allocation)?;
        Ok(Some(BindingPublication {
            store: self,
            update,
            retired: removed,
            nodes,
            next_generation,
        }))
    }
}

impl BindingPublication<'_> {
    /// Call only alongside the already prepared memo/box publication.
    /// The exclusive store borrow prevents intervening publication.
    pub fn publish(self) {
        for key in self.retired {
            self.store.segments.remove(&key);
        }
        self.store.segments.extend(self.update.changed);
        for (key, counts) in self.nodes {
            if counts.flags == [0; 3] {
                self.store.nodes.remove(&key);
            } else {
                self.store.nodes.insert(key, counts);
            }
        }
        self.store.generation = self.next_generation;
    }
}
