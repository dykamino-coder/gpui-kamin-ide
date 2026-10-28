//! Rasterized `<foreignObject>` layers of one document: `data:` URIs of their PNG snapshots,
//! keyed by the hash of the layer content, style, sizes and density. A bounded resource memo:
//! when it holds 64 snapshots, the next insertion starts it afresh.

use std::cell::RefCell;
use std::collections::HashMap;

/// Snapshot capacity before the memo is cleared.
const CAPACITY: usize = 64;

#[derive(Default)]
pub(crate) struct ForeignRasters(RefCell<HashMap<u64, String>>);

impl ForeignRasters {
    /// The remembered snapshot URI of layer `key`.
    pub(crate) fn get(&self, key: u64) -> Option<String> {
        self.0.borrow().get(&key).cloned()
    }

    /// Remember the snapshot URI of layer `key`.
    pub(crate) fn remember(&self, key: u64, uri: String) {
        let mut cache = self.0.borrow_mut();
        if cache.len() >= CAPACITY {
            cache.clear();
        }
        cache.insert(key, uri);
    }
}
