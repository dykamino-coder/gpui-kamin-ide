//! Процессный кэш результатов, не зависящих от документа (растры, разобранные картинки), с
//! бюджетом в БАЙТАХ и вытеснением давно не использованных записей (LRU).
//!
//! Владеет одним механизмом для всех растровых кэшей покраски и `svg/`: вес записи — байты,
//! которые она держит живыми (пиксели растра плюс постоянная накладная доля записи), а при
//! превышении бюджета уходят самые старые по последнему обращению записи — ровно столько,
//! сколько нужно, а не весь кэш. Блокировка держится только на время поиска и вставки:
//! растеризация идёт без неё (`get_or_insert_with`), поэтому два потока могут однажды
//! посчитать одно и то же — результат детерминирован, вторая вставка заменит первую.
//! Запись тяжелее всего бюджета не кэшируется: иначе она вытеснила бы всё остальное.

use gpui::RenderImage;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Мебибайт — единица бюджетов кэшей.
pub(crate) const MIB: usize = 1 << 20;

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
}

/// Кэш с бюджетом в байтах; `const`-конструктор позволяет держать его в `static`.
pub(crate) struct BoundedCache<K, V> {
    budget: usize,
    weigh: fn(&V) -> usize,
    store: Mutex<Option<Store<K, V>>>,
}

impl<K: Eq + Hash, V: Clone> BoundedCache<K, V> {
    /// `budget` — потолок суммарного веса; `weigh` — полезные байты значения.
    pub(crate) const fn new(budget: usize, weigh: fn(&V) -> usize) -> Self {
        Self {
            budget,
            weigh,
            store: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Store<K, V>>> {
        // Под замком не исполняется чужой код, кроме `Hash`/`Eq` ключей и `weigh`; даже
        // после их паники таблица согласована, поэтому отравление не мешает работе.
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Найти запись и отметить её как только что использованную.
    pub(crate) fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut guard = self.lock();
        let store = guard.as_mut()?;
        store.clock += 1;
        let slot = store.map.get_mut(key)?;
        slot.used = store.clock;
        Some(slot.value.clone())
    }

    /// Положить запись; при превышении бюджета вытесняются самые давние записи.
    pub(crate) fn insert(&self, key: K, value: V) {
        let weight = (self.weigh)(&value).saturating_add(ENTRY_OVERHEAD + size_of::<K>());
        if weight > self.budget {
            return;
        }
        let mut guard = self.lock();
        let store = guard.get_or_insert_with(|| Store {
            map: HashMap::new(),
            bytes: 0,
            clock: 0,
        });
        store.clock += 1;
        let slot = Slot {
            value,
            weight,
            used: store.clock,
        };
        if let Some(old) = store.map.insert(key, slot) {
            store.bytes -= old.weight;
        }
        store.bytes += weight;
        if store.bytes > self.budget {
            evict(store, self.budget);
        }
    }

    /// Взять из кэша или создать без замка и запомнить.
    pub(crate) fn get_or_insert_with(&self, key: K, create: impl FnOnce() -> V) -> V {
        if let Some(hit) = self.get(&key) {
            return hit;
        }
        let value = create();
        self.insert(key, value.clone());
        value
    }

    /// Суммарный вес записей.
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.lock().as_ref().map_or(0, |s| s.bytes)
    }

    /// Число записей.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.lock().as_ref().map_or(0, |s| s.map.len())
    }
}

/// Убрать самые давние записи, пока вес не уложится в бюджет. Одна сортировка на
/// вытеснение; только что вставленная запись новее всех и не тяжелее бюджета, поэтому
/// остаётся.
fn evict<K: Eq + Hash, V>(store: &mut Store<K, V>, budget: usize) {
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
    store.map.retain(|_, slot| {
        let keep = slot.used > cutoff;
        if !keep {
            freed += slot.weight;
        }
        keep
    });
    store.bytes -= freed;
}

/// Ключ без своего `Hash` (значения с `f32` внутри): хэшируется подсказкой `hint`, а
/// сравнивается целиком. Равные ключи обязаны давать равную подсказку — её строят из
/// части полей ключа.
#[derive(Clone)]
pub(crate) struct Hinted<K> {
    pub(crate) hint: u64,
    pub(crate) key: K,
}

impl<K: PartialEq> PartialEq for Hinted<K> {
    fn eq(&self, other: &Self) -> bool {
        self.hint == other.hint && self.key == other.key
    }
}

// Ключ с NaN не равен себе и просто не находится — запись уйдёт по вытеснению.
impl<K: PartialEq> Eq for Hinted<K> {}

impl<K> Hash for Hinted<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hint.hash(state);
    }
}

/// Подсказка из хэшируемой части ключа.
pub(crate) fn hint(part: impl Hash) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    part.hash(&mut hasher);
    hasher.finish()
}

/// Байты пикселей всех кадров растра.
pub(crate) fn image_bytes(image: &RenderImage) -> usize {
    (0..image.frame_count())
        .filter_map(|frame| image.as_bytes(frame))
        .map(<[u8]>::len)
        .sum()
}

/// Вес растра, которого может не быть (неразбираемый источник тоже запоминается).
pub(crate) fn optional_image_bytes(image: &Option<Arc<RenderImage>>) -> usize {
    image.as_deref().map_or(0, image_bytes)
}

/// Вес растра.
pub(crate) fn arc_image_bytes(image: &Arc<RenderImage>) -> usize {
    image_bytes(image)
}

#[cfg(test)]
mod tests;
