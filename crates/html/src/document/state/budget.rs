//! Ревизия, на которую документ списывает работу ресурсов и стиля. Одна на состояние:
//! хранилище картинок, шрифты, профили и подстановки делят её aggregate.

use super::DocState;
use crate::budget::RevisionBudget;

impl DocState {
    /// Committed или собираемая ревизия. Состояние без Document (фикстуры, вложенные
    /// документы до B3) использует конечный профиль своего хранилища картинок.
    pub(crate) fn revision_budget(&self) -> RevisionBudget {
        self.budget
            .borrow_mut()
            .get_or_insert_with(|| self.images.borrow().budget().clone())
            .clone()
    }

    /// Установить ревизию сборки до первого прохода, списывающего её work.
    pub(crate) fn admit_revision(&self, revision: RevisionBudget) {
        self.images.borrow_mut().set_budget(revision.clone());
        self.budget.replace(Some(revision));
    }
}
