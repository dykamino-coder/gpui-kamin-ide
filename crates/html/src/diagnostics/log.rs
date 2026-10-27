//! Ограниченный журнал ревизии: фиксированная ёмкость, агрегация повторов,
//! счётчик подавленных и курсор доставки. Без подписчика журнал не растёт.

use super::Diagnostic;
use crate::dom::outcome::DocumentFailure;

/// Начальный профиль ADR2 §5: записей на ревизию.
pub const MAX_RECORDS: usize = 128;
/// Начальный профиль ADR2 §5: байт записей на ревизию.
pub const MAX_BYTES: usize = 64 * 1024;

// Записи фиксированного размера без строк: байтовый предел следует из числа записей.
const _: () = assert!(MAX_RECORDS * std::mem::size_of::<Diagnostic>() <= MAX_BYTES);

/// Порция для embedder: новые ключи с прошлой выдачи и отдельно terminal cause.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticBatch {
    /// Ключ committed revision, к которой относятся записи.
    pub revision: Option<u64>,
    pub records: Vec<Diagnostic>,
    /// Наблюдения, не поместившиеся в журнал (насыщающийся счётчик).
    pub suppressed: u64,
    /// Отказ ревизии хранится вне очереди и не вытесняется переполнением.
    pub terminal: Option<DocumentFailure>,
}

impl DiagnosticBatch {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.suppressed == 0 && self.terminal.is_none()
    }
}

pub(crate) struct DiagnosticLog {
    records: Vec<Diagnostic>,
    /// Записи до этого индекса уже выданы `take`.
    delivered: usize,
    suppressed: u64,
    suppressed_delivered: u64,
}

impl Default for DiagnosticLog {
    /// Ёмкость резервируется до основной работы ревизии.
    fn default() -> Self {
        Self {
            records: Vec::with_capacity(MAX_RECORDS),
            delivered: 0,
            suppressed: 0,
            suppressed_delivered: 0,
        }
    }
}

impl DiagnosticLog {
    /// Учесть `diagnostic.occurrences` наблюдений его ключа.
    pub(crate) fn record(&mut self, diagnostic: Diagnostic) {
        if let Some(known) = self.records.iter_mut().find(|r| r.same_key(&diagnostic)) {
            known.occurrences = known.occurrences.saturating_add(diagnostic.occurrences);
        } else if self.records.len() < MAX_RECORDS {
            self.records.push(diagnostic);
        } else {
            self.suppressed = self
                .suppressed
                .saturating_add(u64::from(diagnostic.occurrences));
        }
    }

    /// Подавленные наблюдения вложенного журнала остаются подавленными.
    pub(crate) fn suppress(&mut self, count: u64) {
        self.suppressed = self.suppressed.saturating_add(count);
    }

    /// Новые записи и подавленные с прошлой выдачи; повтор выданного ключа
    /// только увеличивает его счётчик в `snapshot`.
    pub(crate) fn take(&mut self) -> (Vec<Diagnostic>, u64) {
        let fresh = self.records[self.delivered..].to_vec();
        self.delivered = self.records.len();
        let suppressed = self.suppressed - self.suppressed_delivered;
        self.suppressed_delivered = self.suppressed;
        (fresh, suppressed)
    }

    /// Все записи ревизии с текущими счётчиками; курсор не меняется.
    pub(crate) fn snapshot(&self) -> (Vec<Diagnostic>, u64) {
        (self.records.clone(), self.suppressed)
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.records.capacity()
    }
}
