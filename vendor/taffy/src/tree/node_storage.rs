//! KaminIDE patch: stable generational IDs with payload pages owned by live nodes.
//!
//! SlotMap keeps its small key/location table, so a deleted ID never names a new
//! node. Dense pages hold the large NodeData values. Removing a node moves only
//! the last value and releases each emptied page, including after navigation.

use super::NodeData;
use crate::util::sys::Vec;
use core::ops::{Index, IndexMut};
use slotmap::{DefaultKey, SlotMap};

// Allocation granularity, not a cache limit: all live nodes are stored.
const PAGE_NODES: usize = 64;
type Entry = (DefaultKey, NodeData);

#[derive(Debug, Clone)]
pub(super) struct NodeStorage {
    locations: SlotMap<DefaultKey, usize>,
    pages: Vec<Vec<Entry>>,
}

impl NodeStorage {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        let mut storage = Self {
            locations: SlotMap::with_capacity(capacity),
            pages: Vec::new(),
        };
        storage.reserve(capacity);
        storage
    }

    pub(super) fn len(&self) -> usize {
        self.locations.len()
    }

    pub(super) fn capacity(&self) -> usize {
        let payloads = self.pages.last().map_or(0, |last| {
            (self.pages.len() - 1) * PAGE_NODES + last.capacity()
        });
        self.locations.capacity().min(payloads)
    }

    pub(super) fn reserve(&mut self, additional: usize) {
        self.locations.reserve(additional);
        let nodes = self.len() + additional;
        let needed = (nodes + PAGE_NODES - 1) / PAGE_NODES;
        while self.pages.len() < needed {
            self.pages.push(Vec::new());
        }
        for (index, page) in self.pages.iter_mut().enumerate().take(needed) {
            let wanted = (nodes - index * PAGE_NODES).min(PAGE_NODES);
            page.reserve_exact(wanted.saturating_sub(page.len()));
        }
    }

    pub(super) fn insert(&mut self, value: NodeData) -> DefaultKey {
        let index = self.len();
        if index == self.pages.len() * PAGE_NODES {
            // Independent layout probes often contain just one node. Their
            // first page grows with live payloads instead of reserving 64.
            let capacity = if index == 0 { 0 } else { PAGE_NODES };
            self.pages.push(Vec::with_capacity(capacity));
        }
        let key = self.locations.insert(index);
        let page = &mut self.pages[index / PAGE_NODES];
        if page.len() == page.capacity() {
            let capacity = (page.len() * 2).clamp(1, PAGE_NODES);
            page.reserve_exact(capacity - page.len());
        }
        page.push((key, value));
        key
    }

    pub(super) fn remove(&mut self, key: DefaultKey) -> Option<NodeData> {
        let index = self.locations.remove(key)?;
        let remaining = self.len();
        let last = self.pages[remaining / PAGE_NODES]
            .pop()
            .expect("live node has a payload");
        let removed = if index == remaining {
            last
        } else {
            self.locations[last.0] = index;
            core::mem::replace(
                &mut self.pages[index / PAGE_NODES][index % PAGE_NODES],
                last,
            )
        };
        self.pages
            .truncate((remaining + PAGE_NODES - 1) / PAGE_NODES);
        Some(removed.1)
    }

    pub(super) fn clear(&mut self) {
        self.locations.clear();
        self.pages.clear();
    }

    pub(super) fn contains_key(&self, key: DefaultKey) -> bool {
        self.locations.contains_key(key)
    }

    pub(super) fn get(&self, key: DefaultKey) -> Option<&NodeData> {
        let &index = self.locations.get(key)?;
        Some(&self.pages[index / PAGE_NODES][index % PAGE_NODES].1)
    }

    pub(super) fn get_mut(&mut self, key: DefaultKey) -> Option<&mut NodeData> {
        let &index = self.locations.get(key)?;
        Some(&mut self.pages[index / PAGE_NODES][index % PAGE_NODES].1)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = DefaultKey> + '_ {
        self.locations.keys()
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &NodeData> {
        self.pages
            .iter()
            .flat_map(|page| page.iter().map(|entry| &entry.1))
    }

    #[cfg(feature = "std")]
    pub(super) fn heap_report(&self, sink: &mut dyn FnMut(&'static str, *const u8, usize)) {
        use core::mem::size_of;
        use slotmap::Key;
        let path = "layout/taffy-nodes";
        let stride = size_of::<(usize, u32)>();
        let start = self
            .locations
            .iter()
            .next()
            .map_or(core::ptr::null(), |(key, value)| {
                let index = (key.data().as_ffi() & 0xffff_ffff) as usize;
                (value as *const usize)
                    .cast::<u8>()
                    .wrapping_sub(index * stride)
            });
        sink(path, start, (self.locations.capacity() + 1) * stride);
        sink(
            path,
            self.pages.as_ptr().cast(),
            self.pages.capacity() * size_of::<Vec<Entry>>(),
        );
        for page in &self.pages {
            sink(
                path,
                page.as_ptr().cast(),
                page.capacity() * size_of::<Entry>(),
            );
        }
    }
}

impl Index<DefaultKey> for NodeStorage {
    type Output = NodeData;

    fn index(&self, key: DefaultKey) -> &Self::Output {
        self.get(key).expect("invalid node key")
    }
}

impl IndexMut<DefaultKey> for NodeStorage {
    fn index_mut(&mut self, key: DefaultKey) -> &mut Self::Output {
        self.get_mut(key).expect("invalid node key")
    }
}

#[cfg(test)]
#[path = "node_storage_tests.rs"]
mod tests;
