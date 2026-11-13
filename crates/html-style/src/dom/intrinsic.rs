//! Постоянные intrinsic-размеры узла: неизменяемые входы и локальный dirty-слот.
//! Список детей отделяет слот при COW, материализация помечает цепочку предков.

use crate::style::computed::Computed;
use std::cell::Cell;
use std::rc::Rc;

pub type IntrinsicSizes = Rc<Cell<Option<(f32, f32)>>>;

/// Входы измерения без доступной ширины: она относится к placement, а не min/max.
#[derive(Clone, Debug)]
pub struct IntrinsicInput {
    pub styles: Vec<Rc<Computed>>,
    pub viewport: (f32, f32),
    pub containing: (Option<f32>, Option<f32>),
    pub em: f32,
    pub fonts: u64,
    pub images: u64,
    pub vertical: bool,
}

/// То же поэлементное равенство, что у derive, в порядке цены: скаляры
/// первыми, стиль — сначала по адресу опубликованного снимка. Смена
/// viewport или ширины блока (relayout) отвергается без глубокого
/// сравнения `Computed`, равный по значению новый снимок по-прежнему совпадает.
impl PartialEq for IntrinsicInput {
    fn eq(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && self.containing == other.containing
            && self.em == other.em
            && self.fonts == other.fonts
            && self.images == other.images
            && self.vertical == other.vertical
            && self.styles.len() == other.styles.len()
            && self
                .styles
                .iter()
                .zip(&other.styles)
                .all(|(a, b)| Rc::ptr_eq(a, b) || a == b)
    }
}

#[derive(Default)]
pub(super) struct IntrinsicCache(Cell<Option<Rc<Entry>>>);

#[derive(Debug)]
struct Entry {
    input: IntrinsicInput,
    sizes: IntrinsicSizes,
}

impl std::fmt::Debug for IntrinsicCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("IntrinsicCache")
            .field(&self.entry())
            .finish()
    }
}

impl IntrinsicCache {
    fn entry(&self) -> Option<Rc<Entry>> {
        // Запись неизменяема: Cell передаёт владение указателем, а не выдаёт
        // изменяемый доступ к данным узла. До возврата слот восстановлен.
        let entry = self.0.take();
        self.0.set(entry.clone());
        entry
    }

    pub(super) fn dirty(&self) {
        if let Some(entry) = self.0.take() {
            entry.sizes.set(None);
        }
    }

    pub(super) fn sizes(&self, input: IntrinsicInput) -> IntrinsicSizes {
        if let Some(entry) = self.entry()
            && entry.input == input
        {
            return entry.sizes.clone();
        }
        let sizes = IntrinsicSizes::default();
        self.0.set(Some(Rc::new(Entry {
            input,
            sizes: sizes.clone(),
        })));
        sizes
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for IntrinsicCache {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        let entry = self.0.take();
        let bytes = entry
            .as_ref()
            .map_or(0, |entry| ops.shallow(Rc::as_ptr(entry)));
        self.0.set(entry);
        bytes
    }
}
