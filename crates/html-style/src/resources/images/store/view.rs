//! Чтение хранилища на один согласованный проход стиля, раскладки и покраски: снимок не
//! грузит, не декодирует и не меняет документ — неизвестное остаётся `Pending` до того,
//! как оркестратор выполнит задания перед следующим снимком.

use super::{ImageId, ImageState, ImageStore};

/// Только чтение: состояние запросов и ревизия хранилища на момент снимка.
pub struct ImageReadView<'a> {
    store: &'a ImageStore,
}

impl ImageStore {
    /// Есть запросы, ещё не загруженные или не декодированные оркестратором.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
            || self
                .slots
                .iter()
                .filter_map(|slot| slot.entry.as_ref())
                .any(|entry| matches!(entry.state, ImageState::Pending))
    }

    /// Согласованное чтение состояния на один проход.
    pub fn snapshot(&self) -> ImageReadView<'_> {
        ImageReadView { store: self }
    }
}

impl ImageReadView<'_> {
    /// Состояние запроса; `None` — ручка устарела (запрос освобождён или документ обновлён).
    pub fn state(&self, id: ImageId) -> Option<&ImageState> {
        self.store.entry(id).map(|e| &e.state)
    }

    /// Ревизия хранилища: меняется при каждом изменении состояния.
    pub fn revision(&self) -> u64 {
        self.store.revision
    }

    /// What layout reads of the store: each request with its state and, once
    /// decoded, its natural size, orientation and kind. The revision changes
    /// with any bookkeeping (a new document revision drops and asks again for
    /// every request, a released subscription frees one); these facts change
    /// only when an image a layout may read is answered otherwise, so a result
    /// kept across revisions (`band_flow::HostSlot`) stays valid while they
    /// are equal. Order-free: a sum of per-request hashes.
    pub fn layout_facts(&self) -> u64 {
        use std::sync::atomic::Ordering::Relaxed;
        let store = self.store;
        let [at, kept] = &store.facts;
        let now = store.revision.wrapping_add(1);
        if at.load(Relaxed) == now {
            return kept.load(Relaxed);
        }
        let facts = store
            .slots
            .iter()
            .filter_map(|slot| slot.entry.as_ref())
            .map(entry_facts)
            .fold(0u64, u64::wrapping_add);
        kept.store(facts, Relaxed);
        at.store(now, Relaxed);
        facts
    }
}

fn entry_facts(entry: &super::Entry) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entry.request.hash(&mut hasher);
    match &entry.state {
        ImageState::Pending => 0u8.hash(&mut hasher),
        ImageState::Ready { metadata, .. } => {
            1u8.hash(&mut hasher);
            let n = &metadata.natural;
            for v in [n.w, n.h, n.ratio] {
                v.map(f32::to_bits).hash(&mut hasher);
            }
            metadata.applied_orientation.hash(&mut hasher);
            metadata.vector.hash(&mut hasher);
        }
        ImageState::Failed(error) => {
            2u8.hash(&mut hasher);
            format!("{error:?}").hash(&mut hasher);
        }
    }
    hasher.finish()
}
