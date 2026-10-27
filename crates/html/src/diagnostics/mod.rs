//! Типизированные диагностики ревизии (ADR2 §5): стабильный код, severity, scope,
//! фаза и измерение предела. Свободного текста записи не содержат — его строят
//! представления (`console`, `status`), поэтому в отчёт не попадают разметка,
//! payload или адреса ресурсов. Хранение ограничено (`log`), доставка — по запросу
//! embedder на границе фазы, без callback из глубины движка.

pub mod console;
mod log;
mod status;
#[cfg(test)]
mod tests;

pub use crate::dom::outcome::{FailureScope as Scope, TraversalKind};
pub(crate) use log::DiagnosticLog;
pub use log::{DiagnosticBatch, MAX_BYTES, MAX_RECORDS};
pub use status::DisplayStatus;

use crate::budget::{BudgetError, BudgetExceeded};
use crate::dom::outcome::{DocumentFailure, FailureReason, TraversalLimit};

/// Стабильный машиночитаемый код; строка не меняется вместе с текстом сообщений.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Code {
    TraversalDepth,
    XmlSyntax,
    BudgetExceeded,
    BudgetInvariant,
    /// Предел одного ресурса: байты до декодирования, заявленный растр, кодек.
    ResourceLimit,
    /// Подстановка `var()` стала invalid at computed-value time по пределу реализации.
    SubstitutionLimit,
    /// Задание отменено вместе с устаревшим поколением документа (телеметрия).
    Cancelled,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TraversalDepth => "KAMINIDE_LIMIT_TRAVERSAL_DEPTH",
            Self::XmlSyntax => "KAMINIDE_XML_SYNTAX",
            Self::BudgetExceeded => "KAMINIDE_BUDGET_EXCEEDED",
            Self::BudgetInvariant => "KAMINIDE_BUDGET_INVARIANT",
            Self::ResourceLimit => "KAMINIDE_LIMIT_RESOURCE",
            Self::SubstitutionLimit => "KAMINIDE_LIMIT_SUBSTITUTION",
            Self::Cancelled => "KAMINIDE_CANCELLED",
        }
    }

    pub(crate) fn of(reason: FailureReason) -> Self {
        match reason {
            FailureReason::XmlSyntax { .. } => Self::XmlSyntax,
            FailureReason::Budget(BudgetError::Exceeded(_)) => Self::BudgetExceeded,
            FailureReason::Budget(_) => Self::BudgetInvariant,
        }
    }
}

/// Info — телеметрия без проблемы для пользователя (отмена устаревшего задания);
/// Warning — документ показан со штатным fallback; Error — отказ своей области.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Стадия, на которой обнаружено событие.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Проверка source/budget до разбора.
    Admission,
    /// Синтаксис входа (XML).
    Parse,
    /// Ограниченный проход представления.
    Traversal(TraversalKind),
    /// Загрузка и декодирование ресурса.
    Resource,
    /// Вычисление стиля (подстановки).
    Style,
}

/// Что именно упёрлось в предел.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Measure {
    /// Глубина пути в дереве; `attempted` — фактическая глубина, а не номер тега.
    Depth {
        limit: u64,
        attempted: u64,
    },
    Budget(BudgetExceeded),
    /// Неизменяемый предел одного ресурса или значения, в байтах.
    Ceiling {
        limit: u64,
    },
    /// Результат длиннее предела: `attempted` байт при пределе `limit`.
    Length {
        limit: u64,
        attempted: u64,
    },
    /// Заявленные стороны растра непредставимы.
    Dimensions {
        width: u32,
        height: u32,
    },
}

/// Позиция только если она достоверно известна.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    Source { row: u32, column: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: Code,
    pub severity: Severity,
    pub scope: Scope,
    pub phase: Phase,
    /// Измерение первого наблюдения этого ключа.
    pub measure: Option<Measure>,
    pub location: Option<Location>,
    /// Число наблюдений ключа в ревизии, насыщающееся.
    pub occurrences: u32,
}

impl Diagnostic {
    /// Ключ агрегации повторов (ADR2 §5: code/scope/location).
    fn same_key(&self, other: &Self) -> bool {
        (self.code, self.scope, self.phase, self.location)
            == (other.code, other.scope, other.phase, other.location)
    }
}

impl Diagnostic {
    /// Наблюдение продюсера фазы; scope выставляет владелец журнала.
    pub(crate) fn observed(
        code: Code,
        severity: Severity,
        phase: Phase,
        measure: Option<Measure>,
    ) -> Self {
        Self {
            code,
            severity,
            scope: Scope::Document,
            phase,
            measure,
            location: None,
            occurrences: 1,
        }
    }

    /// Отказ ledger в фазе: aggregate ревизии терминален, live-квота — fallback.
    pub(crate) fn budget(phase: Phase, error: BudgetError) -> Self {
        match error {
            BudgetError::Exceeded(e) => {
                let severity = if e.scope == crate::budget::BudgetScope::Revision {
                    Severity::Error
                } else {
                    Severity::Warning
                };
                Self::observed(
                    Code::BudgetExceeded,
                    severity,
                    phase,
                    Some(Measure::Budget(e)),
                )
            }
            _ => Self::observed(Code::BudgetInvariant, Severity::Error, phase, None),
        }
    }
}

impl From<TraversalLimit> for Diagnostic {
    fn from(limit: TraversalLimit) -> Self {
        Self {
            code: Code::TraversalDepth,
            severity: Severity::Warning,
            scope: limit.scope,
            phase: Phase::Traversal(limit.traversal),
            measure: Some(Measure::Depth {
                limit: limit.limit as u64,
                attempted: limit.attempted as u64,
            }),
            location: None,
            occurrences: 1,
        }
    }
}

impl From<&DocumentFailure> for Diagnostic {
    fn from(failure: &DocumentFailure) -> Self {
        let (phase, measure, location) = match failure.reason {
            FailureReason::XmlSyntax { row, column } => {
                (Phase::Parse, None, Some(Location::Source { row, column }))
            }
            FailureReason::Budget(BudgetError::Exceeded(exceeded)) => {
                (Phase::Admission, Some(Measure::Budget(exceeded)), None)
            }
            FailureReason::Budget(_) => (Phase::Admission, None, None),
        };
        Self {
            code: Code::of(failure.reason),
            severity: Severity::Error,
            scope: failure.scope,
            phase,
            measure,
            location,
            occurrences: 1,
        }
    }
}
