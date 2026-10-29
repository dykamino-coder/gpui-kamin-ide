//! Учёт работы и типизированные отказы подстановок значений CSS.

use crate::budget::{BudgetError, BudgetKind, RevisionBudget};
use std::cell::Cell;

// Пределы в байтах UTF-8 из неизменяемого профиля.
const MAX_OUTPUT: usize = crate::budget::DocumentLimits::VAR_OUTPUT_BYTES as usize;
const MAX_WORK: usize = crate::budget::DocumentLimits::VAR_WORK_BYTES as usize;

/// Какой предел сделал подстановку негодной.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubstitutionLimit {
    /// Результат одного значения длиннее `limit` байт (CSS Variables §3.3).
    Output {
        limit: u64,
        attempted: u64,
    },
    /// Работа одного значения или DAG пользовательских свойств элемента.
    Work {
        limit: u64,
    },
    /// Aggregate подстановок ревизии исчерпан; ревизия завершается отказом.
    Revision(BudgetError),
    Admission(crate::budget::admission::Error),
}

/// Почему значения нет: обычное guaranteed-invalid или предел реализации.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unsubstituted {
    Invalid,
    Limit(SubstitutionLimit),
}

/// Бюджет работы одной области подстановки: значения обычного свойства или всего DAG
/// пользовательских свойств элемента. Каждая единица списывается и с ревизии.
pub(crate) struct Work<'a> {
    left: Cell<usize>,
    revision: &'a RevisionBudget,
    admission: crate::budget::admission::Admission,
    /// Первый предел области: одна причина для журнала документа.
    first: Cell<Option<SubstitutionLimit>>,
}

impl<'a> Work<'a> {
    pub(crate) fn new(revision: &'a RevisionBudget) -> Self {
        Self {
            left: Cell::new(MAX_WORK),
            revision,
            admission: crate::budget::admission::Admission::new(revision.clone()),
            first: Cell::new(None),
        }
    }

    /// Записать первый предел области в журнал документа (invalid at computed time
    /// со штатной семантикой уже применён вызывающим).
    pub(crate) fn finish(self, doc: &crate::document::state::DocumentValues) {
        if let Some(limit) = self.first.get() {
            doc.record_observed(diagnostic(limit));
        }
    }

    fn limit(&self, limit: SubstitutionLimit) -> Unsubstituted {
        if self.first.get().is_none() {
            self.first.set(Some(limit));
        }
        Unsubstituted::Limit(limit)
    }

    pub(crate) fn admission(&self) -> &crate::budget::admission::Admission {
        &self.admission
    }

    pub(crate) fn admit<T>(
        &self,
        result: Result<T, crate::budget::admission::Error>,
    ) -> Result<T, Unsubstituted> {
        result.map_err(|error| {
            self.revision.reject(error.into());
            self.limit(SubstitutionLimit::Admission(error))
        })
    }

    pub(crate) fn spend(&self, bytes: usize) -> Result<(), Unsubstituted> {
        let Some(left) = self.left.get().checked_sub(bytes) else {
            return Err(self.limit(SubstitutionLimit::Work {
                limit: MAX_WORK as u64,
            }));
        };
        self.revision
            .consume(BudgetKind::SubstitutionBytes, bytes as u64)
            .map_err(|error| self.limit(SubstitutionLimit::Revision(error)))?;
        self.left.set(left);
        Ok(())
    }

    pub(crate) fn append(&self, result: &mut String, value: &str) -> Result<(), Unsubstituted> {
        self.fit(result.len(), value.len())?;
        self.spend(value.len())?;
        result.push_str(value);
        Ok(())
    }

    pub(crate) fn fit(&self, len: usize, extra: usize) -> Result<(), Unsubstituted> {
        if extra > MAX_OUTPUT.saturating_sub(len) {
            return Err(self.limit(SubstitutionLimit::Output {
                limit: MAX_OUTPUT as u64,
                attempted: len as u64 + extra as u64,
            }));
        }
        Ok(())
    }
}

fn diagnostic(limit: SubstitutionLimit) -> crate::diagnostics::Diagnostic {
    use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};
    let measure = match limit {
        SubstitutionLimit::Output { limit, attempted } => Measure::Length { limit, attempted },
        SubstitutionLimit::Work { limit } => Measure::Ceiling { limit },
        SubstitutionLimit::Revision(error) => return Diagnostic::budget(Phase::Style, error),
        SubstitutionLimit::Admission(error) => {
            let failure = crate::document::DocumentFailure::from(error);
            let mut diagnostic = Diagnostic::from(&failure);
            diagnostic.phase = Phase::Style;
            return diagnostic;
        }
    };
    Diagnostic::observed(
        Code::SubstitutionLimit,
        Severity::Warning,
        Phase::Style,
        Some(measure),
    )
}
