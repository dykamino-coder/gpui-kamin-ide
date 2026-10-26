//! Чтение хранилища на один согласованный проход стиля, раскладки и покраски: снимок не
//! грузит, не декодирует и не меняет документ — неизвестное остаётся `Pending` до того,
//! как оркестратор выполнит задания перед следующим снимком.

use super::{ImageId, ImageState, ImageStore};

/// Только чтение: состояние запросов и ревизия хранилища на момент снимка.
pub(crate) struct ImageReadView<'a> {
    store: &'a ImageStore,
}

impl ImageStore {
    /// Есть запросы, ещё не загруженные или не декодированные оркестратором.
    pub(crate) fn has_pending(&self) -> bool {
        !self.queue.is_empty()
            || self
                .slots
                .iter()
                .filter_map(|slot| slot.entry.as_ref())
                .any(|entry| matches!(entry.state, ImageState::Pending))
    }

    /// Согласованное чтение состояния на один проход.
    pub(crate) fn snapshot(&self) -> ImageReadView<'_> {
        ImageReadView { store: self }
    }
}

impl ImageReadView<'_> {
    /// Состояние запроса; `None` — ручка устарела (запрос освобождён или документ обновлён).
    pub(crate) fn state(&self, id: ImageId) -> Option<&ImageState> {
        self.store.entry(id).map(|e| &e.state)
    }

    /// Ревизия хранилища: меняется при каждом изменении состояния.
    pub(crate) fn revision(&self) -> u64 {
        self.store.revision
    }
}
