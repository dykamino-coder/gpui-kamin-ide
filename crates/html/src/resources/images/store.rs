//! Хранилище картинок ОДНОГО документа (`DocState::images`): таблица запросов с
//! дедупликацией по разрешённому адресу и параметрам, поколение документа и ревизия
//! состояния, политика доступа, подписки потребителей и приём завершённых заданий.
//!
//! Владение как у Blink (`ImageResourceContent` с observers у документа) и Servo
//! (`ImageCache` на pipeline): документ хранит запросы и подписки, а неизменяемые
//! декодированные значения берутся из внедрённого процессного кэша по содержимому.
//! Изменение состояния отделено от чтения: `ImageReadView` ничего не грузит и не меняет,
//! задания выполняются вне хранилища (`job`), а устаревшие итоги (другое поколение,
//! освобождённый слот, перезапрошенная попытка) отбрасываются.

mod access;
mod job;
mod rasters;
mod resolve;
mod subscriptions;
#[cfg(test)]
mod tests;
mod view;

pub(crate) use access::ResourcePolicy;
pub(crate) use job::ImageServices;
pub(crate) use subscriptions::{ConsumerId, ImageChange, ImageUse};

use super::types::{DecodeOptions, DecodedImage, ImageError, ImageMetadata};
use crate::resources::loader::{ResourceUrl, Revision};
use std::collections::HashMap;
use url::Url;

/// Ручка запроса: слот таблицы и поколение слота (повторно занятый слот — другая ручка).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImageId {
    slot: u32,
    generation: u32,
}

/// Запрос документа: разрешённый адрес без фрагмента и параметры декодирования.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImageRequest {
    pub(crate) url: ResourceUrl,
    pub(crate) options: DecodeOptions,
}

/// Состояние запроса.
#[derive(Clone, Debug)]
pub(crate) enum ImageState {
    Pending,
    Ready {
        metadata: ImageMetadata,
        image: DecodedImage,
    },
    Failed(ImageError),
}

struct Entry {
    request: ImageRequest,
    state: ImageState,
    /// Ревизия содержимого, с которой получено состояние (`None` — ещё не читалось).
    content: Option<Revision>,
    /// Номер последней поставленной попытки: итог прежней попытки устарел.
    attempt: u64,
    subscribers: Vec<(ConsumerId, ImageUse)>,
}

#[derive(Default)]
struct Slot {
    generation: u32,
    entry: Option<Entry>,
}

/// Таблица запросов картинок документа.
pub(crate) struct ImageStore {
    services: ImageServices,
    rasters: rasters::Rasters,
    base: Option<Url>,
    policy: ResourcePolicy,
    generation: u32,
    revision: u64,
    attempts: u64,
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_request: HashMap<ImageRequest, ImageId>,
    queue: Vec<ImageId>,
}

impl Default for ImageStore {
    fn default() -> Self {
        Self::new(ImageServices::process(), None, ResourcePolicy::default())
    }
}

impl ImageStore {
    pub(crate) fn new(services: ImageServices, base: Option<Url>, policy: ResourcePolicy) -> Self {
        Self {
            services,
            rasters: rasters::Rasters::default(),
            base,
            policy,
            generation: 0,
            revision: 0,
            attempts: 0,
            slots: Vec::new(),
            free: Vec::new(),
            by_request: HashMap::new(),
            queue: Vec::new(),
        }
    }

    /// Новая база документа: прежние запросы разрешались от старой и отбрасываются.
    pub(crate) fn set_base(&mut self, base: Option<Url>) {
        if self.base != base {
            self.base = base;
            self.begin_generation();
        }
    }

    /// Разрешить авторский адрес от базы документа; фрагмент — отдельно.
    pub(crate) fn resolve(&self, authored: &str) -> (ResourceUrl, Option<String>) {
        ResourceUrl::resolve(authored, self.base.as_ref())
    }

    /// Ручка запроса; одинаковый запрос документа не грузится дважды.
    pub(crate) fn request(&mut self, request: ImageRequest) -> ImageId {
        if let Some(&id) = self.by_request.get(&request) {
            return id;
        }
        let allowed = self.policy.allows(&request.url);
        let state = if allowed {
            ImageState::Pending
        } else {
            ImageState::Failed(ImageError::AccessDenied)
        };
        let slot = self.free.pop().unwrap_or_else(|| {
            self.slots.push(Slot::default());
            self.slots.len() as u32 - 1
        });
        let id = ImageId {
            slot,
            generation: self.slots[slot as usize].generation,
        };
        self.attempts += 1;
        self.slots[slot as usize].entry = Some(Entry {
            request: request.clone(),
            state,
            content: None,
            attempt: self.attempts,
            subscribers: Vec::new(),
        });
        self.by_request.insert(request, id);
        if allowed {
            self.queue.push(id);
        }
        self.revision += 1;
        id
    }

    /// Перепроверить ревизии прочитанного: изменившийся, появившийся или исчезнувший файл
    /// ставится заново, прежнее состояние держится до нового итога. Число поставленных.
    pub(crate) fn revalidate(&mut self) -> usize {
        let mut queued = 0;
        for (slot, cell) in self.slots.iter_mut().enumerate() {
            let Some(entry) = cell.entry.as_mut() else {
                continue;
            };
            let Some(seen) = &entry.content else { continue };
            if *seen == Revision::Immutable
                || self.services.loader.revision(&entry.request.url) == *seen
            {
                continue;
            }
            self.attempts += 1;
            entry.attempt = self.attempts;
            entry.content = None;
            self.queue.push(ImageId {
                slot: slot as u32,
                generation: cell.generation,
            });
            queued += 1;
        }
        queued
    }

    /// Обновление документа: все запросы и подписки отбрасываются, итоги прежнего
    /// поколения больше не принимаются.
    pub(crate) fn begin_generation(&mut self) {
        self.generation += 1;
        self.rasters.clear();
        for (slot, cell) in self.slots.iter_mut().enumerate() {
            if cell.entry.take().is_some() {
                cell.generation += 1;
                self.free.push(slot as u32);
            }
        }
        self.by_request.clear();
        self.queue.clear();
        self.revision += 1;
    }

    fn entry(&self, id: ImageId) -> Option<&Entry> {
        let cell = self.slots.get(id.slot as usize)?;
        (cell.generation == id.generation).then_some(cell.entry.as_ref()?)
    }

    fn entry_mut(&mut self, id: ImageId) -> Option<&mut Entry> {
        let cell = self.slots.get_mut(id.slot as usize)?;
        (cell.generation == id.generation).then_some(cell.entry.as_mut()?)
    }
}
