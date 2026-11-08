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
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Выведенные группы одного документа.
#[derive(Default)]
pub struct DerivedGroups {
    entries: RefCell<HashMap<Key, Entry>>,
    generation: Cell<u32>,
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
        let generation = self.generation.get();
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get_mut(&key)
            && StyleGroup::<T>::restore(&entry.source).is_some_and(|held| held.ptr_eq(source))
            && let Some(result) = StyleGroup::<T>::restore(&entry.result)
        {
            entry.used = generation;
            return result;
        }
        let mut result = source.clone();
        edit(&mut result);
        entries.insert(
            key,
            Entry {
                source: source.erase(),
                result: result.erase(),
                used: generation,
            },
        );
        result
    }

    /// Начало кадра: записи, которыми не пользовался прошлый кадр, отпускают свои группы.
    pub fn begin_frame(&self) {
        let generation = self.generation.get();
        self.entries
            .borrow_mut()
            .retain(|_, entry| entry.used == generation);
        self.generation.set(generation.wrapping_add(1));
    }

    /// После новой материализации: исходную группу, которую держит только запись, больше
    /// никто не предъявит — запись отпускает её сразу, а не через кадр.
    pub fn release_unowned(&self) {
        self.entries
            .borrow_mut()
            .retain(|_, entry| Rc::strong_count(&entry.source) > 1);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Push;
    struct Other;

    #[test]
    fn one_edit_of_one_group_is_shared() {
        let cache = DerivedGroups::default();
        let base = StyleGroup::new(vec![1]);
        let first = cache.derive::<Push, _>(&base, 2, |v| v.push(2));
        // Another owner handle of the same group hits the same entry.
        let alias = base.clone();
        let second = cache.derive::<Push, _>(&alias, 2, |v| v.push(2));
        drop(alias);
        assert!(first.ptr_eq(&second));
        assert_eq!(&*first, &[1, 2]);
        assert_eq!(&*base, &[1]);
        let third = cache.derive::<Push, _>(&base, 3, |v| v.push(3));
        assert!(!third.ptr_eq(&first));
        let other = cache.derive::<Other, _>(&base, 2, |v| v.push(2));
        assert!(!other.ptr_eq(&first));
        let distinct = StyleGroup::new(vec![1]);
        assert!(
            !cache
                .derive::<Push, _>(&distinct, 2, |v| v.push(2))
                .ptr_eq(&first)
        );
    }

    #[test]
    fn frames_release_unused_entries() {
        let cache = DerivedGroups::default();
        let base = StyleGroup::new(vec![1]);
        let kept = cache.derive::<Push, _>(&base, 2, |v| v.push(2));
        let _dropped = cache.derive::<Push, _>(&base, 3, |v| v.push(3));
        cache.begin_frame();
        assert_eq!(cache.len(), 2);
        assert!(
            cache
                .derive::<Push, _>(&base, 2, |v| v.push(2))
                .ptr_eq(&kept)
        );
        cache.begin_frame();
        assert_eq!(cache.len(), 1);
        let other = StyleGroup::new(vec![5]);
        let _ = cache.derive::<Push, _>(&other, 2, |v| v.push(2));
        assert_eq!(cache.len(), 2);
        drop(other);
        cache.release_unowned();
        assert_eq!(cache.len(), 1);
    }
}
