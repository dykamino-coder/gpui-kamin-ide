//! Byte- and entry-bounded query captures, shared on hits; styles resolve after lookup.

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    mem::size_of,
    rc::Rc,
};

pub(super) const CAPACITY: usize = 4096;
pub(super) const BYTE_BUDGET: usize = 4 * 1024 * 1024;
type Key = (usize, usize);

// KaminIDE patch: bound retained payloads and metadata independently. FIFO eviction
// needs no sorting; oversize queries bypass the cache without evicting useful entries.
pub(super) struct CaptureCache<T> {
    state: RefCell<State<T>>,
}

struct Entry<T> {
    captures: Rc<[T]>,
    bytes: usize,
}

struct State<T> {
    entries: HashMap<Key, Entry<T>>,
    order: VecDeque<Key>,
    bytes: usize,
}

impl<T> Default for CaptureCache<T> {
    fn default() -> Self {
        Self {
            state: RefCell::new(State {
                entries: HashMap::new(),
                order: VecDeque::new(),
                bytes: 0,
            }),
        }
    }
}

impl<T> CaptureCache<T> {
    pub(super) fn get_or_insert_with(
        &self,
        key: Key,
        query: impl FnOnce() -> Vec<T>,
        heap_bytes: impl Fn(&T) -> usize,
    ) -> Rc<[T]> {
        if let Some(hit) = self.state.borrow().entries.get(&key) {
            return Rc::clone(&hit.captures);
        }
        // Run the query without borrowing the cache. Rc<[T]> retains no spare Vec capacity.
        let captures: Rc<[T]> = query().into();
        // Charge the slice, Rc counters, entry and queue key. Hash table/queue spare
        // capacity is bounded by CAPACITY. Shared heap data is conservatively charged
        // once per capture, independent of other owners of the same allocation.
        let bytes = captures.iter().fold(
            size_of::<(Key, Entry<T>)>()
                + size_of::<Key>()
                + 2 * size_of::<usize>()
                + size_of_val(captures.as_ref()),
            |bytes, capture| bytes.saturating_add(heap_bytes(capture)),
        );
        if bytes > BYTE_BUDGET {
            return captures;
        }

        let mut state = self.state.borrow_mut();
        // A reentrant query may have populated this key. Keep its cached result.
        if let Some(hit) = state.entries.get(&key) {
            return Rc::clone(&hit.captures);
        }
        while state.entries.len() >= CAPACITY || state.bytes + bytes > BYTE_BUDGET {
            let oldest = state
                .order
                .pop_front()
                .expect("cached entry has a FIFO key");
            let removed = state.entries.remove(&oldest).expect("FIFO key is cached");
            state.bytes -= removed.bytes;
        }
        state.bytes += bytes;
        state.order.push_back(key);
        state.entries.insert(
            key,
            Entry {
                captures: Rc::clone(&captures),
                bytes,
            },
        );
        captures
    }

    pub(super) fn clear(&self) {
        let mut state = self.state.borrow_mut();
        state.entries.clear();
        state.order.clear();
        state.bytes = 0;
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.state.borrow().entries.len()
    }
}

#[cfg(test)]
#[path = "capture_cache_tests.rs"]
mod tests;
