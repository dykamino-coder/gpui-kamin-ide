//! Выдача диагностик embedder и постоянный статус представления (ADR2 §5).
//! Вызывается хостом между фазами (после build/update или после кадра), а не
//! из layout/paint: движок не вызывает UI callbacks.

use super::Document;
use crate::diagnostics::{DiagnosticBatch, DisplayStatus};

impl Document {
    /// Новые записи ревизии с прошлой выдачи, включая вложенные документы
    /// последнего кадра. Отказ update приходит в `terminal` при каждой выдаче,
    /// пока следующая ревизия не будет принята.
    pub fn take_diagnostics(&self) -> DiagnosticBatch {
        let (records, suppressed) = self.state.take_diagnostics();
        self.batch(records, suppressed)
    }

    /// Все записи текущей ревизии без сдвига курсора выдачи (панель диагностики).
    pub fn diagnostics(&self) -> DiagnosticBatch {
        let (records, suppressed) = self.state.diagnostics_snapshot();
        self.batch(records, suppressed)
    }

    fn batch(
        &self,
        records: Vec<crate::diagnostics::Diagnostic>,
        suppressed: u64,
    ) -> DiagnosticBatch {
        DiagnosticBatch {
            revision: Some(self.key),
            records,
            suppressed,
            terminal: self.last_failure,
        }
    }

    /// Что показано пользователю и почему.
    pub fn display_status(&self) -> DisplayStatus {
        if let Some(failure) = self.last_failure {
            return DisplayStatus::Stale(failure);
        }
        let first_limit = self.traversal_limit();
        let embedded_failure = self.embedded_failure();
        if first_limit.is_none() && embedded_failure.is_none() {
            return DisplayStatus::Complete;
        }
        let (records, suppressed) = self.state.diagnostics_snapshot();
        DisplayStatus::Degraded {
            first_limit,
            embedded_failure,
            problems: records.len(),
            suppressed,
        }
    }

    /// Минимальный постоянный сигнал для host, включая виртуализованный вывод.
    pub fn status_message(&self) -> Option<String> {
        self.display_status().message()
    }
}
