//! What the caches tell DevTools: the bytes each named cache holds (memory
//! domain, kept always: one atomic add per insert or eviction) and, only
//! while the `why` journal records, the keys they let go, so a later miss
//! says `evicted` or `not_admitted` instead of `new_key`.

use super::BoundedCache;
use std::collections::HashMap;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// Bound of `Store::gone`; past it the memory starts again.
const GONE: usize = 1 << 16;

/// Names of the caches in the memory report (`held`); unnamed ones are `resource`.
pub const NAMES: [&str; 3] = ["resource", "gradient_tiles", "rounded_masks"];
/// Bytes the caches of each name hold, kept by their inserts and evictions
/// (lookups do not touch it).
pub(super) static HELD: [AtomicUsize; NAMES.len()] = [const { AtomicUsize::new(0) }; NAMES.len()];

/// Bytes held by the process-wide caches, by name (DevTools memory domain).
pub fn held() -> impl Iterator<Item = (&'static str, usize)> {
    NAMES
        .iter()
        .zip(&HELD)
        .map(|(name, bytes)| (*name, bytes.load(Relaxed)))
}

/// `a == b` for `const` name lookups (`BoundedCache::named`).
pub(super) const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Keep why the key with hash `key` left (`evicted`, `not_admitted`).
pub(super) fn remember(gone: &mut HashMap<u64, &'static str>, key: u64, why: &'static str) {
    if gone.len() >= GONE {
        gone.clear();
    }
    gone.insert(key, why);
}

/// A cache that ends (not a `static`) stops holding its bytes.
impl<K, V> Drop for BoundedCache<K, V> {
    fn drop(&mut self) {
        let store = self.store.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let Some(store) = store {
            HELD[self.slot].fetch_sub(store.bytes, Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{BoundedCache, ENTRY_OVERHEAD};

    fn len(v: &Vec<u8>) -> usize {
        v.len()
    }

    #[test]
    fn a_miss_names_new_evicted_and_not_admitted_keys_at_the_caller() {
        let (_one, _sub) = crate::why::test_reader();
        let one = 100 + ENTRY_OVERHEAD + size_of::<u32>();
        let cache: BoundedCache<u32, Vec<u8>> =
            BoundedCache::new(one * 2, len).named("rounded_masks");
        let line = line!() + 1;
        assert!(cache.lookup(&1, |r| format!("why {r}")) == Err("new_key"));
        for k in 1..=3 {
            cache.insert(k, vec![0; 100]);
        }
        cache.insert(9, vec![0; 10 * one]);
        assert!(cache.get(&3).is_some());
        assert_eq!(cache.lookup(&1, |_| String::new()), Err("evicted"));
        assert_eq!(cache.lookup(&9, |_| String::new()), Err("not_admitted"));
        let j = crate::why::take();
        let stats = &j.caches["rounded_masks"];
        assert_eq!(stats.hits, 1);
        assert_eq!(
            stats
                .misses
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>(),
            [("evicted", 1), ("new_key", 1), ("not_admitted", 1)]
        );
        let first = &stats.examples[0];
        assert_eq!(
            (first.code, first.detail.as_str()),
            ("new_key", "why new_key")
        );
        assert!(
            first.site().ends_with(&format!("cache/why.rs:{line}")),
            "{}",
            first.site()
        );
    }

    #[test]
    fn named_caches_report_the_bytes_they_hold_until_they_end() {
        // Only the tests of this module name `rounded_masks` in this crate; they take turns.
        let _one = crate::why::test_reader().0;
        let held = || {
            super::held()
                .find(|(n, _)| *n == "rounded_masks")
                .unwrap()
                .1
        };
        let before = held();
        let cache: BoundedCache<u32, Vec<u8>> =
            BoundedCache::new(1 << 20, len).named("rounded_masks");
        cache.insert(1, vec![0; 1000]);
        assert_eq!(held(), before + 1000 + ENTRY_OVERHEAD + size_of::<u32>());
        drop(cache);
        assert_eq!(held(), before);
    }
}
