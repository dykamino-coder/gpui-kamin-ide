//! Подписки потребителей на запрос и уведомления об изменении: кто и ради чего читает
//! картинку (размер для раскладки, альфа для обтекания, точки для покраски, факты для
//! свёртки фона), чтобы изменение будило только затронутых. Отписка последнего держателя
//! освобождает запрос документа, но не трогает декодированное значение у других держателей
//! (Gecko `imgRequestProxy`: отписка клиента ≠ уничтожение картинки).

use super::{Entry, ImageId, ImageState, ImageStore};

/// Потребитель картинки внутри документа (узел, слой, правило).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConsumerId(pub u64);

/// Ради чего потребитель читает картинку — от этого зависит, какое изменение его касается.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageUse {
    /// Природный размер и соотношение — раскладка замещаемых.
    Intrinsic,
    /// Альфа — `shape-outside`.
    Alpha,
    /// Точки — покраска.
    Pixels,
    /// Доступность и сплошной цвет — свёртка фонов в стиле.
    Facts,
}

/// Подписка; отдаётся обратно в `ImageStore::release`.
#[derive(Debug, PartialEq, Eq)]
pub struct Subscription {
    id: ImageId,
    consumer: ConsumerId,
    usage: ImageUse,
}

/// Что изменилось у запроса и кого это касается.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageChange {
    pub id: ImageId,
    /// Ревизия хранилища после изменения.
    pub revision: u64,
    pub metadata_changed: bool,
    pub pixels_changed: bool,
    /// Запрос пришёл в конечное состояние (готов или ошибка).
    pub terminal: bool,
    pub consumers: Vec<(ConsumerId, ImageUse)>,
}

impl ImageChange {
    pub(super) fn between(
        id: ImageId,
        revision: u64,
        old: &ImageState,
        new: &ImageState,
        consumers: &[(ConsumerId, ImageUse)],
    ) -> Self {
        let (metadata_changed, pixels_changed) = match (old, new) {
            (
                ImageState::Ready {
                    metadata: a,
                    image: x,
                },
                ImageState::Ready {
                    metadata: b,
                    image: y,
                },
            ) => (a != b, !same_image(x, y)),
            (ImageState::Ready { .. }, _) | (_, ImageState::Ready { .. }) => (true, true),
            _ => (false, false),
        };
        Self {
            id,
            revision,
            metadata_changed,
            pixels_changed,
            terminal: !matches!(new, ImageState::Pending),
            consumers: consumers.to_vec(),
        }
    }
}

fn same_image(a: &super::DecodedImage, b: &super::DecodedImage) -> bool {
    use super::DecodedImage::{Raster, Vector};
    match (a, b) {
        (Raster(a), Raster(b)) => std::sync::Arc::ptr_eq(a, b),
        (Vector(a), Vector(b)) => std::sync::Arc::ptr_eq(a, b),
        _ => false,
    }
}

impl ImageStore {
    /// Подписать потребителя; `None`, если ручка устарела.
    pub fn subscribe(
        &mut self,
        id: ImageId,
        consumer: ConsumerId,
        usage: ImageUse,
    ) -> Option<Subscription> {
        let entry = self.entry_mut(id)?;
        if !entry.subscribers.contains(&(consumer, usage)) {
            entry.subscribers.push((consumer, usage));
        }
        Some(Subscription {
            id,
            consumer,
            usage,
        })
    }

    /// Отписать; без подписчиков запрос освобождается, его ручка устаревает.
    pub fn release(&mut self, subscription: Subscription) {
        let Some(entry) = self.entry_mut(subscription.id) else {
            return;
        };
        entry
            .subscribers
            .retain(|s| *s != (subscription.consumer, subscription.usage));
        if entry.subscribers.is_empty() {
            self.free_slot(subscription.id);
        }
    }

    fn free_slot(&mut self, id: ImageId) {
        let cell = &mut self.slots[id.slot as usize];
        let Some(Entry { request, .. }) = cell.entry.take() else {
            return;
        };
        cell.generation += 1;
        self.by_request.remove(&request);
        self.free.push(id.slot);
        self.revision += 1;
    }
}
