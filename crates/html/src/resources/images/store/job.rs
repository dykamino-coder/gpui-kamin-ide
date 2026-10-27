//! Работа загрузки и декодирования вне хранилища: задание несёт всё нужное (адрес,
//! параметры, поколение документа, номер попытки, ревизию учёта и отмену поколения), а
//! выполняется над внедрёнными службами без доступа к документу — синхронно сейчас, на
//! рабочем потоке позже (I-7). Результат возвращается хранилищу и принимается, только если
//! не устарел; отвергнутый итог возвращает свои квоты при drop (RAII), отмена — не ошибка.

use super::{ImageChange, ImageId, ImageState, ImageStore};
use crate::budget::{BudgetKind, LiveReservation, RevisionBudget};
use crate::resources::images::admission::Admission;
use crate::resources::images::cache::{Decoded, DecodedImageCache};
use crate::resources::images::types::{DecodeOptions, ImageError, ImageLimit};
use crate::resources::loader::{
    Cancellation, LoadError, ResourceLoader, ResourceUrl, Revision, SyncLoader, fetch,
};
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

/// Поставленный запрос держит live-квоту задания документа до выполнения или отмены.
pub(super) struct Queued {
    pub(super) id: ImageId,
    pub(super) task: LiveReservation,
}

/// Задание на загрузку и декодирование одного запроса.
pub(crate) struct LoadJob {
    pub(crate) generation: u32,
    pub(crate) id: ImageId,
    pub(crate) attempt: u64,
    pub(crate) url: ResourceUrl,
    pub(crate) options: DecodeOptions,
    budget: RevisionBudget,
    cancel: Cancellation,
    _task: LiveReservation,
}

/// Итог задания: ревизия прочитанного содержимого, результат и допуск точек документом.
pub(crate) struct CompletedImage {
    pub(crate) generation: u32,
    pub(crate) id: ImageId,
    pub(crate) attempt: u64,
    pub(crate) revision: Revision,
    pub(crate) result: Decoded,
    admission: Option<Admission>,
}

impl LoadJob {
    /// Прочитать байты с допуском ревизии и декодировать через общий кэш.
    pub(crate) fn run(&self, services: &ImageServices) -> CompletedImage {
        let loaded = fetch(&*services.loader, &self.url, &self.budget, &self.cancel);
        let (revision, (result, admission)) = match loaded {
            Ok(_) if self.cancel.is_cancelled() => (Revision::Absent, cancelled()),
            Ok(loaded) => {
                let document = self.budget.document();
                let decoded = services.cache.decode(loaded.bytes, self.options, document);
                (loaded.revision, decoded)
            }
            Err(LoadError::Cancelled) => (Revision::Absent, cancelled()),
            Err(error) => (
                services.loader.revision(&self.url),
                (Err(load_error(error)), None),
            ),
        };
        CompletedImage {
            generation: self.generation,
            id: self.id,
            attempt: self.attempt,
            revision,
            result,
            admission,
        }
    }
}

fn cancelled() -> (Decoded, Option<Admission>) {
    (Err(ImageError::Cancelled), None)
}

fn load_error(error: LoadError) -> ImageError {
    match error {
        LoadError::UnsupportedScheme(scheme) => ImageError::UnsupportedScheme(scheme),
        LoadError::InvalidUrl | LoadError::NotFound | LoadError::Io(_) => ImageError::Io,
        LoadError::TooLarge { limit } => ImageError::LimitExceeded(ImageLimit::ResourceBytes {
            limit: limit as u64,
        }),
        LoadError::Budget(error) => ImageError::LimitExceeded(ImageLimit::Budget(error)),
        LoadError::Cancelled => ImageError::Cancelled,
    }
}

impl Drop for ImageStore {
    /// Закрытое хранилище (заменённая revision) отменяет свои выданные задания.
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl ImageStore {
    /// Ревизия, которой принадлежат следующие задания; прежние задания устаревают.
    pub(crate) fn set_budget(&mut self, budget: RevisionBudget) {
        self.budget = budget;
        self.begin_generation();
    }

    pub(crate) fn budget(&self) -> &RevisionBudget {
        &self.budget
    }

    /// Новые наблюдения пределов и отмен с прошлой выдачи.
    pub(crate) fn take_diagnostics(&self) -> (Vec<crate::diagnostics::Diagnostic>, u64) {
        self.journal().take()
    }

    /// Журнал согласован после паники записи: в нём нет частично записанных ключей.
    fn journal(&self) -> std::sync::MutexGuard<'_, crate::diagnostics::DiagnosticLog> {
        self.diagnostics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn observe(&self, error: &ImageError) {
        if let Some(diagnostic) = crate::resources::images::admission::diagnostic(error) {
            self.journal().record(diagnostic);
        }
    }

    /// Поставить запрос: live-квота задания документа резервируется до работы;
    /// отказ — типизированная неудача запроса, а не вечное ожидание.
    pub(super) fn enqueue(&mut self, id: ImageId) {
        let task = self.budget.document().reserve(BudgetKind::PendingTasks, 1);
        match task {
            Ok(task) => self.queue.push(Queued { id, task }),
            Err(error) => {
                let error = ImageError::LimitExceeded(ImageLimit::Budget(error));
                self.observe(&error);
                if let Some(entry) = self.entry_mut(id) {
                    entry.state = ImageState::Failed(error);
                }
            }
        }
    }

    /// Задания для поставленных запросов; выполнять их можно без хранилища.
    pub(crate) fn take_jobs(&mut self) -> Vec<LoadJob> {
        let queue = std::mem::take(&mut self.queue);
        queue
            .into_iter()
            .filter_map(|Queued { id, task }| {
                let entry = self.entry(id)?;
                Some(LoadJob {
                    generation: self.generation,
                    id,
                    attempt: entry.attempt,
                    url: entry.request.url.clone(),
                    options: entry.request.options,
                    budget: self.budget.clone(),
                    cancel: self.cancel.clone(),
                    _task: task,
                })
            })
            .collect()
    }

    /// Службы, над которыми выполняются задания.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Задания пока выполняются внутри хранилища")
    )]
    pub(crate) fn services(&self) -> &ImageServices {
        &self.services
    }

    /// Принять итог задания; устаревший итог отбрасывается (`None`) вместе с его квотами.
    pub(crate) fn apply_completed(&mut self, done: CompletedImage) -> Option<ImageChange> {
        // Отмена устаревшего задания — телеметрия; прочие отказы — только принятого итога.
        if let Err(error @ ImageError::Cancelled) = &done.result {
            self.observe(error);
        }
        if done.generation != self.generation {
            return None;
        }
        let revision = self.revision + 1;
        let failed = done.result.as_ref().err().cloned();
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
        entry.admission = done.admission;
        entry.content = Some(done.revision);
        self.revision = revision;
        if let Some(error) = failed {
            self.observe(&error);
        }
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
