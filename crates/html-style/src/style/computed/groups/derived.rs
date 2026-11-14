//! Группы, выведенные из общей группы одной правкой used-стадии: общая группа и правка дают
//! общий результат. Раскладка правит used-слой каждого элемента (`SharedStyle::used_mut`), и
//! одна и та же правка одной общей группы у тысяч соседей давала тысячи равных копий.
//!
//! Stylo делит целые вычисленные стили между похожими соседями (`style/sharing/mod.rs`) и
//! повторно использует reset-структуры по узлу правил (`style/rule_cache.rs`); здесь та же
//! мысль на уровне группы: ключ — владелец исходной группы, вид правки и её значение. Кэш
//! принадлежит документу и живёт между кадрами: повторный кадр получает те же группы без копий.
//! Запись держит исходную группу, поэтому её адрес не может достаться другой группе.

use super::StyleGroup;
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

/// Выведенные группы одного документа. Карта выделяется с первой правкой и отпускается,
/// когда пустеет: значения документа без правок держат один пустой указатель.
#[derive(Default)]
pub struct DerivedGroups(Cell<Option<Box<Entries>>>);

#[derive(Default)]
struct Entries {
    map: HashMap<Key, Entry>,
    /// Номер кадра, которым помечаются использованные записи.
    generation: u32,
}

#[derive(PartialEq, Eq, Hash)]
struct Key {
    source: usize,
    edit: TypeId,
    value: u64,
}

/// Владельцы исходной группы и результата без типа: запись не выделяет памяти сверх карты.
struct Entry {
    source: Rc<dyn Any>,
    result: Rc<dyn Any>,
    used: u32,
}

impl DerivedGroups {
    /// Группа `source` после правки `edit` вида `E` со значением `value`. Одна и та же
    /// правка одной группы отдаёт один общий результат. `edit` обязана зависеть только от
    /// исходной группы и `value`.
    pub fn derive<E: 'static, T: Clone + 'static>(
        &self,
        source: &StyleGroup<T>,
        value: u64,
        edit: impl FnOnce(&mut T),
    ) -> StyleGroup<T> {
        let key = Key {
            source: source.addr(),
            edit: TypeId::of::<(E, T)>(),
            value,
        };
        let hit = |entry: &Entry| {
            StyleGroup::<T>::restore(&entry.source)
                .filter(|held| held.ptr_eq(source))
                .and_then(|_| StyleGroup::<T>::restore(&entry.result))
        };
        self.cached(key, hit, || {
            let mut result = source.clone();
            edit(&mut result);
            (source.erase(), result.erase(), result)
        })
    }

    /// Значение `source` после правки `edit` вида `E` со значением `value`: то же правило,
    /// что у групп ([`Self::derive`]), для целого общего значения (used-стиль соседей с
    /// одним вычисленным стилем и одной правкой — один стиль, Stylo `style/sharing`).
    /// `edit` может сама выводить группы через этот кэш.
    pub fn derive_shared<E: 'static, V: Clone + 'static>(
        &self,
        source: &Rc<V>,
        value: u64,
        edit: impl FnOnce(&mut V),
    ) -> Rc<V> {
        let key = Key {
            source: Rc::as_ptr(source) as *const () as usize,
            edit: TypeId::of::<(E, V)>(),
            value,
        };
        let hit = |entry: &Entry| {
            let held = entry.source.clone().downcast::<V>().ok()?;
            Rc::ptr_eq(&held, source)
                .then(|| entry.result.clone().downcast::<V>().ok())
                .flatten()
        };
        self.cached(key, hit, || {
            let mut result = V::clone(source);
            edit(&mut result);
            let result = Rc::new(result);
            (
                source.clone() as Rc<dyn Any>,
                result.clone() as Rc<dyn Any>,
                result,
            )
        })
    }

    /// Результат записи `key`, если `hit` его узнаёт, иначе новый от `make` (исходный и
    /// результат без типа и сам результат). Карта не занята во время `make`: правка может
    /// выводить через этот же кэш.
    fn cached<R>(
        &self,
        key: Key,
        hit: impl FnOnce(&Entry) -> Option<R>,
        make: impl FnOnce() -> (Rc<dyn Any>, Rc<dyn Any>, R),
    ) -> R {
        let mut entries = self.0.take().unwrap_or_default();
        let generation = entries.generation;
        if let Some(entry) = entries.map.get_mut(&key)
            && let Some(result) = hit(entry)
        {
            entry.used = generation;
            self.0.set(Some(entries));
            return result;
        }
        self.0.set(Some(entries));
        let (source, erased, result) = make();
        let mut entries = self.0.take().unwrap_or_default();
        let used = entries.generation;
        entries.map.insert(
            key,
            Entry {
                source,
                result: erased,
                used,
            },
        );
        self.0.set(Some(entries));
        result
    }

    /// Оставить записи, для которых `keep` верно; пустая карта отпускается.
    fn retain(&self, next_frame: bool, mut keep: impl FnMut(&Entry, u32) -> bool) {
        if let Some(mut entries) = self.0.take() {
            let generation = entries.generation;
            entries.map.retain(|_, entry| keep(entry, generation));
            if next_frame {
                entries.generation = generation.wrapping_add(1);
            }
            if !entries.map.is_empty() {
                self.0.set(Some(entries));
            }
        }
    }

    /// Начало кадра: записи, которыми не пользовался прошлый кадр, отпускают свои группы.
    pub fn begin_frame(&self) {
        self.retain(true, |entry, generation| entry.used == generation);
    }

    /// После новой материализации: исходную группу, которую держит только запись, больше
    /// никто не предъявит — запись отпускает её сразу, а не через кадр.
    pub fn release_unowned(&self) {
        self.retain(false, |entry, _| Rc::strong_count(&entry.source) > 1);
    }

    /// Отпустить все записи (последний документ этих значений отпущен).
    pub fn clear(&self) {
        drop(self.0.take());
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        let entries = self.0.take();
        let len = entries.as_ref().map_or(0, |entries| entries.map.len());
        self.0.set(entries);
        len
    }
}
#[cfg(test)]
mod tests;
