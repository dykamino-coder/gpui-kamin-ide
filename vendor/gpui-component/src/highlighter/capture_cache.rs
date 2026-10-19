//! Bounded query captures; theme-dependent styles are resolved after lookup.

use std::{cell::RefCell, collections::HashMap};

pub(super) const CAPACITY: usize = 4096;

// KaminIDE patch: empty query results spend the same cache budget as tokens.
pub(super) struct CaptureCache<T> {
    entries: RefCell<HashMap<(usize, usize), Vec<T>>>,
}

impl<T> Default for CaptureCache<T> {
    fn default() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
        }
    }
}

impl<T: Clone> CaptureCache<T> {
    pub(super) fn get_or_insert_with(
        &self,
        key: (usize, usize),
        query: impl FnOnce() -> Vec<T>,
    ) -> Vec<T> {
        if let Some(hit) = self.entries.borrow().get(&key) {
            return hit.clone();
        }
        let captures = query();
        let mut entries = self.entries.borrow_mut();
        if entries.len() >= CAPACITY {
            entries.clear();
        }
        entries.insert(key, captures.clone());
        captures
    }

    pub(super) fn clear(&self) {
        self.entries.borrow_mut().clear();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}
