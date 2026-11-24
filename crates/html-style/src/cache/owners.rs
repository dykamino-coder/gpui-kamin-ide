//! Entries that die with their owner: a cache whose key names an owner (the
//! governor of a decoded file) drops the entries of an owner that is gone at
//! its next use after an owner went, instead of keeping them until the byte
//! budget pushes them out. A closed document takes its decoded images and
//! their textures along; nothing is swept on a timer.

use super::Store;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Owners gone so far; a cache sweeps when it has not seen the latest one.
static GONE: AtomicU64 = AtomicU64::new(0);

/// An owner went (a document closed): the caches check their keys again.
pub fn owner_gone() {
    GONE.fetch_add(1, Relaxed);
}

/// Drop the entries whose owner is gone, once per owner departure.
pub(super) fn sweep<K, V>(store: &mut Store<K, V>, alive: fn(&K) -> bool, held: usize) {
    let gone = GONE.load(Relaxed);
    if store.seen == gone {
        return;
    }
    store.seen = gone;
    let mut freed = 0;
    store.map.retain(|key, slot| {
        let keep = alive(key);
        if !keep {
            freed += slot.weight;
        }
        keep
    });
    store.bytes -= freed;
    super::HELD[held].fetch_sub(freed, Relaxed);
}
