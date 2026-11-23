//! The first box of a source element by its key, in O(1) without a hash table.
//!
//! Keys are a document counter from one up (`dom::NodeKey`), so a dense
//! vector from the smallest key takes 8 B per node. A key far past the
//! end of the vector (another counter, synthetic boxes) goes to a sparse table,
//! so that the vector does not bloat.

use crate::dom::BoxId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ByNode {
    base: u64,
    dense: Vec<BoxId>,
    more: HashMap<u64, BoxId>,
    /// Only fragmented source nodes need additional boxes.
    fragments: HashMap<u64, Vec<BoxId>>,
}

impl ByNode {
    fn index(&self, key: u64) -> Option<usize> {
        let at = usize::try_from(key.checked_sub(self.base)?).ok()?;
        (at < self.dense.len()).then_some(at)
    }

    pub(super) fn get(&self, key: u64) -> Option<BoxId> {
        self.index(key)
            .map(|at| self.dense[at])
            .filter(|id| id.is_some())
            .or_else(|| self.more.get(&key).copied())
    }

    /// Remember the box if the key has no first one yet.
    pub(super) fn insert_first(&mut self, key: u64, id: BoxId) {
        if self.get(key).is_some() {
            return;
        }
        if self.dense.is_empty() && self.more.is_empty() {
            self.base = key;
        }
        if let Some(at) = self.index(key) {
            self.dense[at] = id;
            return;
        }
        // Growth in order (a typical parse) goes to the vector; a far key to the table.
        let slack = self.dense.len().max(1024) as u64;
        if key >= self.base && key - self.base < self.dense.len() as u64 + slack {
            let at = (key - self.base) as usize;
            self.dense.resize(at + 1, BoxId::NONE);
            self.dense[at] = id;
        } else {
            self.more.entry(key).or_insert(id);
        }
    }

    pub(super) fn insert_box(&mut self, key: u64, id: BoxId) {
        self.insert_first(key, id);
        if self.get(key) != Some(id) {
            let fragments = self.fragments.entry(key).or_default();
            if !fragments.contains(&id) {
                fragments.push(id);
            }
        }
    }

    pub(super) fn all(&self, key: u64) -> impl Iterator<Item = BoxId> + '_ {
        self.get(key).into_iter().chain(
            self.fragments
                .get(&key)
                .into_iter()
                .flat_map(|ids| ids.iter().copied()),
        )
    }

    /// Drop the growth slack once the boxes of a document are in.
    pub(super) fn fit(&mut self) {
        self.dense.shrink_to_fit();
        self.more.shrink_to_fit();
        self.fragments.shrink_to_fit();
    }

    /// Write the key's box over the previous one.
    pub(super) fn set(&mut self, key: u64, id: BoxId) {
        self.fragments.remove(&key);
        self.more.remove(&key);
        if let Some(at) = self.index(key) {
            self.dense[at] = id;
            return;
        }
        self.insert_first(key, id);
    }

    /// Remove one fragment, promoting the next live fragment when necessary.
    pub(super) fn remove_if(&mut self, key: u64, id: BoxId) {
        let mut promote = None;
        let first = self.get(key) == Some(id);
        if let Some(ids) = self.fragments.get_mut(&key) {
            ids.retain(|&other| other != id);
            if first {
                promote = ids.first().copied();
                if promote.is_some() {
                    ids.remove(0);
                }
            }
            if ids.is_empty() {
                self.fragments.remove(&key);
            }
        }
        if first {
            if let Some(at) = self.index(key) {
                self.dense[at] = promote.unwrap_or(BoxId::NONE);
                self.more.remove(&key);
            } else if let Some(next) = promote {
                self.more.insert(key, next);
            } else {
                self.more.remove(&key);
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.dense.clear();
        self.more.clear();
        self.fragments.clear();
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        ops.buffer(self.dense.as_ptr(), self.dense.capacity())
            + ops.table::<(u64, BoxId)>(self.more.capacity())
            + ops.table::<(u64, Vec<BoxId>)>(self.fragments.capacity())
            + self
                .fragments
                .values()
                .map(|ids| ops.buffer(ids.as_ptr(), ids.capacity()))
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(slot: u32) -> BoxId {
        BoxId {
            slot,
            generation: 1,
        }
    }

    #[test]
    fn dense_keys_stay_in_the_vector_and_far_ones_spill() {
        let mut map = ByNode::default();
        for key in 5..105 {
            map.insert_first(key, id(key as u32));
        }
        map.insert_first(7, id(999));
        assert_eq!(map.get(7), Some(id(7)), "the first box stays");
        map.insert_first(u64::MAX - 1, id(1));
        map.insert_first(1, id(2));
        assert_eq!(map.get(u64::MAX - 1), Some(id(1)));
        assert_eq!(map.get(1), Some(id(2)));
        assert_eq!(map.dense.len(), 100);
        map.remove_if(7, id(8));
        assert_eq!(map.get(7), Some(id(7)));
        map.remove_if(7, id(7));
        assert_eq!(map.get(7), None);
        assert_eq!(map.get(4), None);
    }

    #[test]
    fn dense_growth_preserves_sparse_primary_and_fragment_retirement() {
        let mut map = ByNode::default();
        map.insert_first(10, id(10));
        map.insert_first(2048, id(1));
        assert_eq!(map.more.get(&2048), Some(&id(1)));
        for key in [1000, 1900, 2100] {
            map.insert_first(key, id(key as u32));
        }
        assert!(map.index(2048).is_some());
        assert_eq!(map.get(2048), Some(id(1)));
        map.insert_first(2048, id(999));
        map.insert_box(2048, id(2));
        assert_eq!(map.all(2048).collect::<Vec<_>>(), vec![id(1), id(2)]);
        map.remove_if(2048, id(1));
        assert_eq!(map.get(2048), Some(id(2)));
        assert!(!map.more.contains_key(&2048));
        map.insert_box(2048, id(3));
        map.set(2048, id(4));
        assert_eq!(map.all(2048).collect::<Vec<_>>(), vec![id(4)]);
        map.remove_if(2048, id(2));
        assert_eq!(map.get(2048), Some(id(4)));
        map.remove_if(2048, id(4));
        assert_eq!(map.get(2048), None);
    }
}
