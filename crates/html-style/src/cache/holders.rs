//! Values held outside the cache. A cache of shared values (`Arc`) lets an
//! evicted value live as long as someone holds it: evicting an entry that the
//! frame which drew it still holds frees no byte, and the next frame builds it
//! again. A cache whose values have holders (`with_holders`) keeps such an
//! entry until it is let go, and the budget bounds the bytes alive only for
//! the cache; a released entry goes with the next eviction. Without it a frame
//! whose rasters outweigh the budget evicts, in least-recently-used order, the
//! very entries the next frame asks for first, and every frame misses them all.

use super::BoundedCache;
use std::hash::Hash;
use std::sync::Arc;

impl<K: Eq + Hash, V: Clone> BoundedCache<K, V> {
    /// `held` — the value is held by someone besides the cache: while it is,
    /// eviction keeps the entry and does not count its bytes as freeable.
    pub const fn with_holders(mut self, held: fn(&V) -> bool) -> Self {
        self.held = held;
        self
    }
}

/// The shared value is held outside the cache too (`with_holders`).
pub fn held_elsewhere<T: ?Sized>(value: &Option<Arc<T>>) -> bool {
    value.as_ref().is_some_and(|v| Arc::strong_count(v) > 1)
}

#[cfg(test)]
mod tests {
    use super::super::{BoundedCache, ENTRY_OVERHEAD};
    use super::held_elsewhere;
    use std::sync::Arc;

    type Bytes = Option<Arc<Vec<u8>>>;

    fn len(v: &Bytes) -> usize {
        v.as_ref().map_or(0, |v| v.len())
    }

    fn weight(n: usize) -> usize {
        n + ENTRY_OVERHEAD + size_of::<u32>()
    }

    #[test]
    fn an_entry_held_elsewhere_is_not_evicted_until_it_is_let_go() {
        let cache: BoundedCache<u32, Bytes> =
            BoundedCache::new(weight(100) * 2, len).with_holders(held_elsewhere);
        // A frame draws three rasters, heavier together than the budget.
        let frame: Vec<Bytes> = (0..3)
            .map(|k| cache.get_or_insert_with(k, || Some(Arc::new(vec![0; 100]))))
            .collect();
        for k in 0..3 {
            assert!(cache.get(&k).is_some(), "held key {k} stays");
        }
        // The next frame asks for them again: all hit, none is built anew.
        for k in 0..3 {
            let again = cache.get_or_insert_with(k, || unreachable!("key {k} rebuilt"));
            assert!(Arc::ptr_eq(
                again.as_ref().unwrap(),
                frame[k as usize].as_ref().unwrap()
            ));
        }
        drop(frame);
        // Released, the oldest go with the next insert and the budget holds again.
        cache.insert(3, Some(Arc::new(vec![0; 100])));
        assert!(cache.bytes() <= weight(100) * 2, "{}", cache.bytes());
        assert!(cache.get(&3).is_some());
    }

    #[test]
    fn without_holders_the_budget_evicts_values_still_held() {
        let cache: BoundedCache<u32, Bytes> = BoundedCache::new(weight(100) * 2, len);
        let _frame: Vec<Bytes> = (0..3)
            .map(|k| cache.get_or_insert_with(k, || Some(Arc::new(vec![0; 100]))))
            .collect();
        assert!(cache.get(&0).is_none(), "the oldest went though it is held");
    }
}
