//! Ревизия, на которую документ списывает работу ресурсов и стиля, и наблюдения её
//! пределов. Одна ревизия на состояние: хранилище картинок, шрифты, профили и подстановки
//! делят её aggregate; отказы приходят в журнал документа с его областью.

use super::DocState;
use crate::budget::RevisionBudget;
use crate::diagnostics::Diagnostic;

impl DocState {
    /// Committed или собираемая ревизия. Состояние без Document (фикстуры, вложенные
    /// документы без ревизии родителя) использует конечный профиль своего хранилища картинок.
    pub(crate) fn revision_budget(&self) -> RevisionBudget {
        let installed = self.budget.borrow().clone();
        installed.unwrap_or_else(|| self.images.borrow().budget().clone())
    }

    /// Установить ревизию сборки и базу её ресурсов до первого прохода, списывающего work.
    pub(crate) fn admit_revision(&self, revision: RevisionBudget, base: Option<&url::Url>) {
        let mut images = self.images.borrow_mut();
        images.set_budget(revision.clone());
        images.set_base(base.cloned());
        drop(images);
        self.budget.replace(Some(revision));
    }

    /// Наблюдение продюсера фазы в области этого документа.
    pub(crate) fn record_observed(&self, mut diagnostic: Diagnostic) {
        diagnostic.scope = self.traversal.scope();
        self.record_diagnostic(diagnostic);
    }

    /// Предел или отмена синхронной загрузки шрифта/профиля.
    pub(crate) fn record_load_failure(&self, error: &crate::resources::loader::LoadError) {
        if let Some(diagnostic) = crate::resources::loader::diagnostic(error) {
            self.record_observed(diagnostic);
        }
    }

    /// Наблюдения заданий картинок переходят в журнал документа на границе выдачи.
    pub(super) fn absorb_resources(&self) {
        let (records, suppressed) = self.images.borrow().take_diagnostics();
        for record in records {
            self.record_observed(record);
        }
        self.diagnostics.borrow_mut().suppress(suppressed);
    }
}
