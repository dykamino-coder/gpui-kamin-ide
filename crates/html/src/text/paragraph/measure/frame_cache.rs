//! Два поколения измерений документа: рабочий набор предыдущего кадра
//! сохраняется целиком, неиспользованные записи удаляются на следующем.

use std::collections::HashMap;

pub(in crate::text::paragraph) struct FrameCache<V> {
    current: HashMap<u64, V>,
    previous: HashMap<u64, V>,
}

impl<V> Default for FrameCache<V> {
    fn default() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
        }
    }
}

#[cfg(feature = "heap-report")]
impl<V> FrameCache<V> {
    /// Таблицы обоих поколений и то, что держат значения (`value`), для
    /// отчёта памяти по владельцам (docs/memprof.md).
    pub(super) fn heap_size(
        &self,
        ops: &mut kamin_heap::Ops,
        mut value: impl FnMut(&V, &mut kamin_heap::Ops) -> usize,
    ) -> usize {
        let mut total = 0;
        for map in [&self.current, &self.previous] {
            total += ops.table::<(u64, V)>(map.capacity());
            for item in map.values() {
                total += value(item, ops);
            }
        }
        total
    }
}

impl<V> FrameCache<V> {
    pub(super) fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }

    pub(in crate::text::paragraph) fn get(&mut self, key: &u64) -> Option<&V> {
        if let Some(value) = self.previous.remove(key) {
            self.current.insert(*key, value);
        }
        self.current.get(key)
    }

    pub(in crate::text::paragraph) fn insert(&mut self, key: u64, value: V) {
        self.previous.remove(&key);
        self.current.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entire_live_working_set_survives_beyond_old_limits() {
        let mut cache = FrameCache::default();
        for key in 0..12_000 {
            cache.insert(key, key + 1);
        }
        for _ in 0..3 {
            cache.begin_frame();
            for key in 0..12_000 {
                assert_eq!(cache.get(&key), Some(&(key + 1)));
            }
        }
        cache.begin_frame();
        for key in 6000..12_000 {
            assert!(cache.get(&key).is_some());
        }
        cache.begin_frame();
        assert!(cache.get(&0).is_none());
        assert_eq!(cache.get(&6000), Some(&6001));
        cache.begin_frame();
        cache.begin_frame();
        assert!(cache.get(&6000).is_none());
    }

    #[test]
    fn replacing_constraint_result_uses_current_value() {
        let mut cache = FrameCache::default();
        cache.insert(1, 10);
        cache.begin_frame();
        cache.insert(1, 20);
        // Новый результат заменяет также запись прошлого поколения.
        assert_eq!(cache.get(&1), Some(&20));
    }
}
