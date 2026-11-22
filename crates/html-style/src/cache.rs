//! Процессный кэш результатов, не зависящих от документа (растры, разобранные картинки,
//! декодированные ресурсы), с бюджетом в БАЙТАХ и вытеснением давно не использованных
//! записей (LRU).
//!
//! Один механизм для растровых кэшей покраски и `svg/` и для декодированных картинок
//! `resources`: вес записи — байты, которые она держит живыми (пиксели плюс постоянная
//! накладная доля записи), а при превышении бюджета уходят самые старые по последнему
//! обращению записи — ровно столько, сколько нужно, а не весь кэш. Блокировка держится
//! только на время поиска и вставки: вычисление значения идёт без неё
//! (`get_or_insert_with`), поэтому два потока могут однажды посчитать одно и то же —
//! результат детерминирован, вторая вставка заменит первую. Запись тяжелее всего бюджета
//! не кэшируется: иначе она вытеснила бы всё остальное. Вытесненное значение живёт, пока
//! его держат (`Arc`), кэш лишь перестаёт его отдавать.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Мебибайт — единица бюджетов кэшей.
pub const MIB: usize = 1 << 20;

/// Накладная доля записи сверх полезных байт: ключ, слот таблицы, счётчики. Не даёт
/// пустым записям (`None` для неразбираемых источников) копиться без счёта.
const ENTRY_OVERHEAD: usize = 64;

struct Slot<V> {
    value: V,
    weight: usize,
    used: u64,
}

struct Store<K, V> {
    map: HashMap<K, Slot<V>>,
    bytes: usize,
    clock: u64,
    /// While the DevTools `why` journal records: hashes of keys the cache
    /// let go and why (`evicted`, `not_admitted`), so a later miss names it.
    gone: HashMap<u64, &'static str>,
}

mod why;
use why::{HELD, remember, same};
pub use why::{NAMES, held};

/// Кэш с бюджетом в байтах; `const`-конструктор позволяет держать его в `static`.
pub struct BoundedCache<K, V> {
    #[cfg(feature = "work-counters")]
    paint: bool,
    /// The cache's name in the `why` journal.
    name: &'static str,
    /// Its place in `NAMES` and `HELD`.
    slot: usize,
    budget: usize,
    weigh: fn(&V) -> usize,
    key_weight: fn(&K) -> usize,
    store: Mutex<Option<Store<K, V>>>,
}

impl<K: Eq + Hash, V: Clone> BoundedCache<K, V> {
    /// `budget` — потолок суммарного веса; `weigh` — полезные байты значения.
    pub const fn new(budget: usize, weigh: fn(&V) -> usize) -> Self {
        Self {
            #[cfg(feature = "work-counters")]
            paint: false,
            name: NAMES[0],
            slot: 0,
            budget,
            weigh,
            key_weight: |_| 0,
            store: Mutex::new(None),
        }
    }

    /// Name the cache in the DevTools `why` journal and memory report; the
    /// name is one of `NAMES` (checked when the constant is built).
    pub const fn named(mut self, name: &'static str) -> Self {
        let mut slot = 0;
        while !same(NAMES[slot], name) {
            slot += 1; // past the end: a name missing from `NAMES` fails the build
        }
        self.name = NAMES[slot];
        self.slot = slot;
        self
    }

    /// Учитывать выделенную память ключа (разметка, удерживаемые CPU-точки).
    pub const fn with_key_weight(mut self, weigh: fn(&K) -> usize) -> Self {
        self.key_weight = weigh;
        self
    }

    /// Категория диагностического наблюдения, без состояния в обычной сборке.
    pub const fn for_paint(self) -> Self {
        #[cfg(feature = "work-counters")]
        {
            let mut cache = self;
            cache.paint = true;
            cache
        }
        #[cfg(not(feature = "work-counters"))]
        {
            self
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Store<K, V>>> {
        // Под замком не исполняется чужой код, кроме `Hash`/`Eq` ключей и `weigh`; даже
        // после их паники таблица согласована, поэтому отравление не мешает работе.
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Найти запись и отметить её как только что использованную.
    #[track_caller]
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.lookup(key, |_| String::new()).ok()
    }

    /// `get`, or why it missed: `new_key`, `evicted` (the budget let it go)
    /// or `not_admitted` (heavier than the budget). The reason is known only
    /// while the `why` journal records (`new_key` otherwise); the miss goes
    /// into the journal at the caller with `explain(reason)` as its detail.
    #[track_caller]
    pub fn lookup<Q>(
        &self,
        key: &Q,
        explain: impl FnOnce(&'static str) -> String,
    ) -> Result<V, &'static str>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let observed = crate::journal::enabled();
        let (result, reason) = {
            let mut guard = self.lock();
            match guard.as_mut() {
                None => (None, "new_key"),
                Some(store) => {
                    store.clock += 1;
                    match store.map.get_mut(key) {
                        Some(slot) => {
                            slot.used = store.clock;
                            (Some(slot.value.clone()), "")
                        }
                        None if observed => (
                            None,
                            store.gone.get(&hint(key)).copied().unwrap_or("new_key"),
                        ),
                        None => (None, "new_key"),
                    }
                }
            }
        };
        if observed {
            match result {
                Some(_) => crate::journal::cache_hit(self.name),
                None => crate::journal::cache_miss(self.name, reason, || explain(reason)),
            }
        }
        crate::work::cache!(result, ResourceCacheHits, ResourceCacheMisses);
        #[cfg(feature = "work-counters")]
        if self.paint {
            crate::work::cache!(result, PaintCacheHits, PaintCacheMisses);
        }
        result.ok_or(reason)
    }

    /// Положить запись; при превышении бюджета вытесняются самые давние записи.
    pub fn insert(&self, key: K, value: V) {
        let weight = (self.weigh)(&value)
            .saturating_add((self.key_weight)(&key))
            .saturating_add(ENTRY_OVERHEAD + size_of::<K>());
        let observed = crate::journal::enabled();
        let mut guard = self.lock();
        let store = guard.get_or_insert_with(|| Store {
            map: HashMap::new(),
            bytes: 0,
            clock: 0,
            gone: HashMap::new(),
        });
        if weight > self.budget {
            if observed {
                remember(&mut store.gone, hint(&key), "not_admitted");
            }
            return;
        }
        store.clock += 1;
        let slot = Slot {
            value,
            weight,
            used: store.clock,
        };
        let before = store.bytes;
        if let Some(old) = store.map.insert(key, slot) {
            store.bytes -= old.weight;
        }
        store.bytes += weight;
        if store.bytes > self.budget {
            evict(store, self.budget, observed);
        }
        let held = &HELD[self.slot];
        match store.bytes >= before {
            true => held.fetch_add(store.bytes - before, Relaxed),
            false => held.fetch_sub(before - store.bytes, Relaxed),
        };
    }

    /// Взять из кэша или создать без замка и запомнить.
    #[track_caller]
    pub fn get_or_insert_with(&self, key: K, create: impl FnOnce() -> V) -> V {
        if let Ok(hit) = self.lookup(&key, |_| String::new()) {
            return hit;
        }
        let value = create();
        self.insert(key, value.clone());
        value
    }

    /// Суммарный вес записей.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.lock().as_ref().map_or(0, |s| s.bytes)
    }

    /// Число записей.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().as_ref().map_or(0, |s| s.map.len())
    }
}

/// Убрать самые давние записи, пока вес не уложится в бюджет. Одна сортировка на
/// вытеснение; только что вставленная запись новее всех и не тяжелее бюджета, поэтому
/// остаётся.
fn evict<K: Eq + Hash, V>(store: &mut Store<K, V>, budget: usize, observed: bool) {
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

/// Число в ключе: конечные значения сохраняются точно, оба нуля совпадают,
/// все NaN образуют одно значение. Бесконечности различаются по знаку.
/// Сырые `f32` не подходят для `Eq`: NaN не равен себе.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatKey(u32);

impl From<f32> for FloatKey {
    fn from(value: f32) -> Self {
        Self(if value == 0.0 {
            0
        } else if value.is_nan() {
            f32::NAN.to_bits()
        } else {
            value.to_bits()
        })
    }
}

/// Ключ без своего `Hash`: хэшируется подсказкой `hint`, сравнивается целиком.
/// Подсказка участвует в равенстве; её строят из канонических полей ключа.
#[derive(Clone, PartialEq, Eq)]
pub struct Hinted<K> {
    pub hint: u64,
    pub key: K,
}

impl<K> Hash for Hinted<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hint.hash(state);
    }
}

/// Подсказка из хэшируемой части ключа.
pub fn hint(part: impl Hash) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    part.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "work-counters"))]
mod work_tests {
    #[test]
    fn observes_empty_miss_and_hit_without_repeating_lookup() {
        use crate::work::{Counter, Phase, Session};
        let session = Session::begin();
        let cache = super::BoundedCache::new(1024, |_: &u8| 1).for_paint();
        assert_eq!(cache.get(&1u8), None);
        cache.insert(1, 9);
        assert_eq!(cache.get(&1), Some(9));
        let counts = session.finish().0[Phase::PipelineOther as usize];
        assert_eq!(counts[Counter::ResourceCacheMisses as usize], 1);
        assert_eq!(counts[Counter::ResourceCacheHits as usize], 1);
        assert_eq!(counts[Counter::PaintCacheMisses as usize], 1);
        assert_eq!(counts[Counter::PaintCacheHits as usize], 1);
    }
}
