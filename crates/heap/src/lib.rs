//! Память по владельцам: каждый владелец по запросу меряет блоки кучи, которыми
//! владеет (механизм MallocSizeOf из Servo/Gecko, свой трейт). Размер — тот,
//! что запрошен у аллокатора (`Layout::size`), как у счётчиков аудита стенда,
//! поэтому сумма отчётов сверяется с `live_requested_bytes` без поправок.
//!
//! `Rc`/`Arc` считаются один раз за обход (`Ops::once`): общий блок
//! принадлежит тому владельцу, кто дошёл до него первым. Если обходу дан
//! приёмник блоков, каждый измеренный блок сообщается с указателем — так
//! стенд сверяет отчёт с живыми блоками пробника (DMD: reported/unreported).
//! Обход ничего не стоит, пока его не вызвали: полей-счётчиков нет.

use impls::counted_header;
use std::collections::{BTreeMap, HashSet};

/// Состояние одного обхода: текущий путь отчёта, счёт байт по путям,
/// уже встреченные общие блоки и необязательный приёмник блоков.
pub struct Ops<'a> {
    path: &'static str,
    tally: BTreeMap<&'static str, u64>,
    seen: HashSet<usize>,
    sink: Option<&'a mut dyn FnMut(&'static str, *const u8, usize)>,
}

impl Default for Ops<'_> {
    fn default() -> Self {
        Self {
            path: "unnamed",
            tally: BTreeMap::new(),
            seen: HashSet::new(),
            sink: None,
        }
    }
}

impl<'a> Ops<'a> {
    /// Обход с приёмником измеренных блоков (путь, указатель, байты).
    pub fn with_sink(sink: &'a mut dyn FnMut(&'static str, *const u8, usize)) -> Self {
        Self {
            sink: Some(sink),
            ..Self::default()
        }
    }

    /// Блок кучи `bytes` по адресу `pointer` (нулевой — адрес неизвестен) на
    /// текущем пути.
    pub fn block(&mut self, pointer: *const u8, bytes: usize) -> usize {
        if bytes == 0 {
            return 0;
        }
        *self.tally.entry(self.path).or_default() += bytes as u64;
        if let Some(sink) = self.sink.as_mut() {
            sink(self.path, pointer, bytes);
        }
        bytes
    }

    /// Блоки, измеренные внутри `measure`, относятся к `path`.
    pub fn at<R>(&mut self, path: &'static str, measure: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.path, path);
        let result = measure(self);
        self.path = outer;
        result
    }

    /// Первая встреча общего блока за этот обход.
    pub fn once<T: ?Sized>(&mut self, pointer: *const T) -> bool {
        self.seen.insert(pointer as *const u8 as usize)
    }

    /// Блок `Rc`/`Arc` без содержимого значения (его куча не измеряется).
    pub fn shallow<T>(&mut self, value: *const T) -> usize {
        if !self.once(value) {
            return 0;
        }
        let align = align_of::<T>().max(align_of::<usize>());
        let header = counted_header(align);
        let bytes = (header + size_of::<T>()).next_multiple_of(align);
        self.block(value.cast::<u8>().wrapping_sub(header), bytes)
    }

    /// Счёт байт по путям.
    pub fn report(&self) -> Report {
        Report {
            entries: self
                .tally
                .iter()
                .map(|(path, bytes)| ((*path).to_owned(), *bytes))
                .collect(),
        }
    }
    /// Буфер вида `Vec` (`capacity` элементов `T` по адресу `pointer`) без содержимого.
    pub fn buffer<T>(&mut self, pointer: *const T, capacity: usize) -> usize {
        self.block(pointer.cast(), capacity * size_of::<T>())
    }

    /// Хэш-таблица hashbrown на `capacity` записей `T` (адрес неизвестен).
    pub fn table<T>(&mut self, capacity: usize) -> usize {
        self.block(std::ptr::null(), table_bytes(capacity, size_of::<T>()))
    }
}

/// Байты одной хэш-таблицы hashbrown (std `HashMap`/`HashSet`, `FxHashMap`):
/// корзины данных, выровненные до 16, плюс байт управления на корзину и группа.
pub fn table_bytes(capacity: usize, entry: usize) -> usize {
    if capacity == 0 {
        return 0;
    }
    let buckets = if capacity < 8 {
        capacity + 1
    } else {
        capacity / 7 * 8
    };
    (buckets * entry).next_multiple_of(16) + buckets + 16
}

/// Блоки кучи, которыми владеет значение (без него самого).
pub trait HeapSize {
    fn heap_size(&self, ops: &mut Ops) -> usize;
}

/// Значения без кучи.
#[macro_export]
macro_rules! no_heap {
    ($($t:ty),* $(,)?) => {
        $(impl $crate::HeapSize for $t {
            #[inline]
            fn heap_size(&self, _: &mut $crate::Ops) -> usize {
                0
            }
        })*
    };
}

/// Сумма полей, владеющих кучей; поля без кучи не перечисляются.
#[macro_export]
macro_rules! heap_fields {
    ($t:ty { $($field:tt),* $(,)? }) => {
        impl $crate::HeapSize for $t {
            fn heap_size(&self, ops: &mut $crate::Ops) -> usize {
                0 $(+ $crate::HeapSize::heap_size(&self.$field, ops))*
            }
        }
    };
}

mod impls;

/// Дерево отчёта «путь → байты», как у about:memory (`page/dom/elements`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    entries: BTreeMap<String, u64>,
}

impl Report {
    pub fn add(&mut self, path: &str, bytes: usize) {
        *self.entries.entry(path.into()).or_default() += bytes as u64;
    }
    /// Добавляет счёт другого отчёта.
    pub fn merge(&mut self, other: &Report) {
        for (path, bytes) in &other.entries {
            *self.entries.entry(path.clone()).or_default() += bytes;
        }
    }
    pub fn entries(&self) -> &BTreeMap<String, u64> {
        &self.entries
    }
    pub fn total(&self) -> u64 {
        self.entries.values().sum()
    }
}

#[cfg(test)]
mod tests;
