//! Первая коробка исходного элемента по его ключу за O(1) без хэш-таблицы.
//!
//! Ключи — счётчик документа подряд от единицы (`dom::NodeKey`), поэтому
//! плотный вектор от наименьшего ключа занимает 8 Б на узел. Ключ далеко за
//! концом вектора (чужой счётчик, синтетика) уходит в разреженную таблицу,
//! чтобы не раздувать вектор.

use crate::dom::BoxId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ByNode {
    base: u64,
    dense: Vec<BoxId>,
    more: HashMap<u64, BoxId>,
}

impl ByNode {
    fn index(&self, key: u64) -> Option<usize> {
        let at = usize::try_from(key.checked_sub(self.base)?).ok()?;
        (at < self.dense.len()).then_some(at)
    }

    pub(super) fn get(&self, key: u64) -> Option<BoxId> {
        match self.index(key) {
            Some(at) => Some(self.dense[at]).filter(|id| id.is_some()),
            None => self.more.get(&key).copied(),
        }
    }

    /// Запомнить коробку, если у ключа ещё нет первой.
    pub(super) fn insert_first(&mut self, key: u64, id: BoxId) {
        if self.dense.is_empty() && self.more.is_empty() {
            self.base = key;
        }
        if let Some(at) = self.index(key) {
            if !self.dense[at].is_some() {
                self.dense[at] = id;
            }
            return;
        }
        // Рост подряд (типичный разбор) — в вектор; далёкий ключ — в таблицу.
        let slack = self.dense.len().max(1024) as u64;
        if key >= self.base && key - self.base < self.dense.len() as u64 + slack {
            let at = (key - self.base) as usize;
            self.dense.resize(at + 1, BoxId::NONE);
            self.dense[at] = id;
        } else {
            self.more.entry(key).or_insert(id);
        }
    }

    /// Записать коробку ключа поверх прежней.
    pub(super) fn set(&mut self, key: u64, id: BoxId) {
        if let Some(at) = self.index(key) {
            self.dense[at] = id;
            return;
        }
        self.more.remove(&key);
        self.insert_first(key, id);
    }

    /// Забыть ключ, если его первая коробка — `id`.
    pub(super) fn remove_if(&mut self, key: u64, id: BoxId) {
        match self.index(key) {
            Some(at) if self.dense[at] == id => self.dense[at] = BoxId::NONE,
            Some(_) => {}
            None => {
                if self.more.get(&key) == Some(&id) {
                    self.more.remove(&key);
                }
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.dense.clear();
        self.more.clear();
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        ops.buffer(self.dense.as_ptr(), self.dense.capacity())
            + ops.table::<(u64, BoxId)>(self.more.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(slot: u32) -> BoxId {
        BoxId {
            slot,
            generation: 1,
        }
    }

    #[test]
    fn dense_keys_stay_in_the_vector_and_far_ones_spill() {
        let mut map = ByNode::default();
        for key in 5..105 {
            map.insert_first(key, id(key as u32));
        }
        map.insert_first(7, id(999));
        assert_eq!(map.get(7), Some(id(7)), "the first box stays");
        map.insert_first(u64::MAX - 1, id(1));
        map.insert_first(1, id(2));
        assert_eq!(map.get(u64::MAX - 1), Some(id(1)));
        assert_eq!(map.get(1), Some(id(2)));
        assert_eq!(map.dense.len(), 100);
        map.remove_if(7, id(8));
        assert_eq!(map.get(7), Some(id(7)));
        map.remove_if(7, id(7));
        assert_eq!(map.get(7), None);
        assert_eq!(map.get(4), None);
    }
}
