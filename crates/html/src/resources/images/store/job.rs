//! Работа загрузки и декодирования вне хранилища: задание несёт всё нужное (адрес,
//! параметры, поколение документа и номер попытки), а выполняется над внедрёнными
//! службами без доступа к документу — синхронно сейчас, на рабочем потоке позже (I-7).
//! Результат возвращается хранилищу и принимается, только если не устарел.

use super::{ImageChange, ImageId, ImageState, ImageStore};
use crate::resources::images::cache::{Decoded, DecodedImageCache};
use crate::resources::images::types::{DecodeOptions, ImageError};
use crate::resources::loader::{LoadError, ResourceLoader, ResourceUrl, Revision, SyncLoader};
use std::sync::Arc;

/// Внедряемые службы: загрузчик байт и процессный кэш декодированного.
#[derive(Clone)]
pub(crate) struct ImageServices {
    pub(crate) loader: Arc<dyn ResourceLoader>,
    pub(crate) cache: Arc<DecodedImageCache>,
}

impl ImageServices {
    /// Службы процесса: синхронный загрузчик и общий кэш.
    pub(crate) fn process() -> Self {
        Self {
            loader: Arc::new(SyncLoader),
            cache: DecodedImageCache::process(),
        }
    }
}

/// Задание на загрузку и декодирование одного запроса.
#[derive(Clone, Debug)]
pub(crate) struct LoadJob {
    pub(crate) generation: u32,
    pub(crate) id: ImageId,
    pub(crate) attempt: u64,
    pub(crate) url: ResourceUrl,
    pub(crate) options: DecodeOptions,
}

/// Итог задания: ревизия прочитанного содержимого и результат декодирования.
#[derive(Clone, Debug)]
pub(crate) struct CompletedImage {
    pub(crate) generation: u32,
    pub(crate) id: ImageId,
    pub(crate) attempt: u64,
    pub(crate) revision: Revision,
    pub(crate) result: Decoded,
}

impl LoadJob {
    /// Прочитать байты и декодировать через общий кэш.
    pub(crate) fn run(&self, services: &ImageServices) -> CompletedImage {
        let (revision, result) = match services.loader.load(&self.url) {
            Ok(loaded) => (
                loaded.revision,
                services.cache.decode(loaded.bytes, self.options),
            ),
            Err(error) => (services.loader.revision(&self.url), Err(load_error(error))),
        };
        CompletedImage {
            generation: self.generation,
            id: self.id,
            attempt: self.attempt,
            revision,
            result,
        }
    }
}

fn load_error(error: LoadError) -> ImageError {
    match error {
        LoadError::UnsupportedScheme(scheme) => ImageError::UnsupportedScheme(scheme),
        LoadError::InvalidUrl | LoadError::NotFound | LoadError::Io(_) => ImageError::Io,
        LoadError::TooLarge { .. } => ImageError::LimitExceeded,
    }
}

impl ImageStore {
    /// Задания для поставленных запросов; выполнять их можно без хранилища.
    pub(crate) fn take_jobs(&mut self) -> Vec<LoadJob> {
        let queue = std::mem::take(&mut self.queue);
        queue
            .into_iter()
            .filter_map(|id| {
                let entry = self.entry(id)?;
                Some(LoadJob {
                    generation: self.generation,
                    id,
                    attempt: entry.attempt,
                    url: entry.request.url.clone(),
                    options: entry.request.options,
                })
            })
            .collect()
    }

    /// Службы, над которыми выполняются задания.
    pub(crate) fn services(&self) -> &ImageServices {
        &self.services
    }

    /// Принять итог задания; устаревший итог отбрасывается (`None`).
    pub(crate) fn apply_completed(&mut self, done: CompletedImage) -> Option<ImageChange> {
        if done.generation != self.generation {
            return None;
        }
        let revision = self.revision + 1;
        let entry = self.entry_mut(done.id)?;
        if entry.attempt != done.attempt {
            return None;
        }
        let state = match done.result {
            Ok((image, metadata)) => ImageState::Ready { metadata, image },
            Err(error) => ImageState::Failed(error),
        };
        let change =
            ImageChange::between(done.id, revision, &entry.state, &state, &entry.subscribers);
        entry.state = state;
        entry.content = Some(done.revision);
        self.revision = revision;
        Some(change)
    }

    /// Синхронный проход: выполнить все поставленные задания и принять их итоги.
    pub(crate) fn complete_pending(&mut self) -> Vec<ImageChange> {
        let services = self.services.clone();
        let done: Vec<_> = self
            .take_jobs()
            .iter()
            .map(|job| job.run(&services))
            .collect();
        done.into_iter()
            .filter_map(|done| self.apply_completed(done))
            .collect()
    }
}
