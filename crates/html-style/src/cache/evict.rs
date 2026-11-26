//! Eviction of a bounded cache: the oldest entries go until the weight fits
//! the budget.

use super::{Store, hint, remember};
use std::hash::Hash;

/// Drop the oldest entries until the weight fits `budget`. One sort per
/// eviction; the entry just inserted is newer than all and no heavier than
/// the budget, so it stays. While observed, the keys let go are kept so a
/// later miss names them (`evicted`).
pub(super) fn evict<K: Eq + Hash, V>(store: &mut Store<K, V>, budget: usize, observed: bool) {
    crate::work::time!(_evict, CacheEvictNs);
    let mut ages: Vec<(u64, usize)> = store.map.values().map(|s| (s.used, s.weight)).collect();
    ages.sort_unstable_by_key(|&(used, _)| used);
    let mut excess = store.bytes - budget;
    let mut cutoff = 0;
    for (used, weight) in ages {
        if excess == 0 {
            break;
        }
        excess = excess.saturating_sub(weight);
        cutoff = used;
    }
    let mut freed = 0;
    let gone = &mut store.gone;
    store.map.retain(|key, slot| {
        let keep = slot.used > cutoff;
        if !keep {
            freed += slot.weight;
            if observed {
                remember(gone, hint(key), "evicted");
            }
        }
        keep
    });
    store.bytes -= freed;
}
