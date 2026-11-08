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

type Entries = HashMap<Key, Entry>;

/// Выведенные группы одного документа. Карта выделяется с первой правкой и отпускается,
/// когда пустеет: значения документа без правок держат только счётчики и указатель.
#[derive(Default)]
pub struct DerivedGroups {
    entries: Cell<Option<Box<Entries>>>,
    generation: Cell<u32>,
    /// Живые документы этих значений (`hold_document`).
    documents: Cell<u32>,
}

impl DerivedGroups {
    /// Документ этих значений создан: его дерево владеет выведенными группами.
    pub fn hold_document(&self) {
        self.documents.set(self.documents.get() + 1);
    }

    /// Документ отпущен. Значения документа переживают его в отложенных кадрах; когда
    /// отпущен последний документ этих значений, записи отпускают его группы сразу, а не
    /// вместе со значениями.
    pub fn release_document(&self) {
        let left = self.documents.get().saturating_sub(1);
        self.documents.set(left);
        if left == 0 {
            drop(self.entries.take());
        }
    }
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
        // Карта вынута на время правки: `edit` правит одну группу и в кэш не входит.
        let mut entries = self.entries.take().unwrap_or_default();
        if let Some(entry) = entries.get_mut(&key)
            && StyleGroup::<T>::restore(&entry.source).is_some_and(|held| held.ptr_eq(source))
            && let Some(result) = StyleGroup::<T>::restore(&entry.result)
        {
            entry.used = generation;
            self.entries.set(Some(entries));
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
        self.entries.set(Some(entries));
        result
    }

    /// Оставить записи, для которых `keep` верно; пустая карта отпускается.
    fn retain(&self, keep: impl FnMut(&Key, &mut Entry) -> bool) {
        if let Some(mut entries) = self.entries.take() {
            entries.retain(keep);
            if !entries.is_empty() {
                self.entries.set(Some(entries));
            }
        }
    }

    /// Начало кадра: записи, которыми не пользовался прошлый кадр, отпускают свои группы.
    pub fn begin_frame(&self) {
        let generation = self.generation.get();
        self.retain(|_, entry| entry.used == generation);
        self.generation.set(generation.wrapping_add(1));
    }

    /// После новой материализации: исходную группу, которую держит только запись, больше
    /// никто не предъявит — запись отпускает её сразу, а не через кадр.
    pub fn release_unowned(&self) {
        self.retain(|_, entry| Rc::strong_count(&entry.source) > 1);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        let entries = self.entries.take();
        let len = entries.as_ref().map_or(0, |entries| entries.len());
        self.entries.set(entries);
        len
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

    #[test]
    fn the_last_document_releases_entries_while_values_live() {
        let cache = DerivedGroups::default();
        let base = StyleGroup::new(vec![1]);
        cache.hold_document();
        let _ = cache.derive::<Push, _>(&base, 2, |v| v.push(2));
        cache.hold_document();
        cache.release_document();
        assert_eq!(cache.len(), 1);
        cache.release_document();
        assert_eq!(cache.len(), 0);
    }
}
