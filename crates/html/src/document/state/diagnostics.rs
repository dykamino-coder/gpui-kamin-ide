//! Журнал диагностик ревизии внутри DocState. Продюсеры фаз только пишут в
//! ограниченный журнал; выдача — по запросу embedder между фазами.

use super::DocState;
use crate::diagnostics::Diagnostic;

impl DocState {
    /// Записать наблюдение; без подписчика журнал остаётся в своих пределах.
    pub(crate) fn record_diagnostic(&self, diagnostic: Diagnostic) {
        self.diagnostics.borrow_mut().record(diagnostic);
    }

    /// Новые записи вложенных документов кадра переходят в журнал владельца.
    /// Вложенный документ пересобирается кадром, поэтому повтор его ключа лишь
    /// увеличивает счётчик уже известной записи.
    fn absorb_children(&self) {
        self.absorb_resources();
        for child in self.observed_children() {
            let (records, suppressed) = child.take_diagnostics();
            for record in records {
                self.record_diagnostic(record);
            }
            self.diagnostics.borrow_mut().suppress(suppressed);
        }
    }

    /// Новые записи (включая вложенные документы) и подавленные с прошлой выдачи.
    pub(crate) fn take_diagnostics(&self) -> (Vec<Diagnostic>, u64) {
        self.absorb_children();
        self.diagnostics.borrow_mut().take()
    }

    /// Все записи ревизии с текущими счётчиками.
    pub(crate) fn diagnostics_snapshot(&self) -> (Vec<Diagnostic>, u64) {
        self.absorb_children();
        self.diagnostics.borrow().snapshot()
    }
}
