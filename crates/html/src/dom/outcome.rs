//! Компактный исход ревизии: отказ либо ограниченное представление целого DOM.

/// Причина отказа без исходной разметки и адресов ресурсов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    XmlSyntax { row: u32, column: u32 },
    Budget(crate::budget::BudgetError),
    Allocation,
    CapacityOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionStatus {
    Complete,
    Degraded,
    Unchanged,
    Rejected,
}

/// Отказ корневой ревизии либо отдельно отображаемого вложенного документа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureScope {
    Document,
    EmbeddedDocument,
}

/// Проход, который сохранил DOM, но не смог обработать более глубоких детей.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversalKind {
    BoxBuild,
    Layout,
    Selectors,
    ShadowTree,
    Counters,
    Slots,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraversalLimit {
    pub scope: FailureScope,
    pub traversal: TraversalKind,
    pub limit: usize,
    pub attempted: usize,
}

impl TraversalLimit {
    pub fn code(self) -> &'static str {
        crate::diagnostics::Code::TraversalDepth.as_str()
    }
}

impl std::fmt::Display for TraversalLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Достигнут предел представления ({}); более глубокие дети не отображены. DOM сохранён полностью",
            self.limit
        )
    }
}

/// Первый предел ревизии; все наблюдения — в `Document::take_diagnostics`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildReport {
    /// Ключ запрошенного содержимого; прямой DOM-разбор не имеет ключа Document.
    pub revision: Option<u64>,
    pub retained_revision: Option<u64>,
    pub status: RevisionStatus,
    pub degradation: Option<TraversalLimit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentFailure {
    pub scope: FailureScope,
    pub reason: FailureReason,
    pub report: BuildReport,
}

impl DocumentFailure {
    pub(crate) fn new(reason: FailureReason) -> Self {
        Self {
            scope: FailureScope::Document,
            reason,
            report: BuildReport {
                revision: None,
                retained_revision: None,
                status: RevisionStatus::Rejected,
                degradation: None,
            },
        }
    }

    /// Стабильный код для host/headless, без разбора текста сообщения.
    pub fn code(&self) -> &'static str {
        crate::diagnostics::Code::of(self.reason).as_str()
    }
}

impl std::fmt::Display for DocumentFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let subject = if self.report.retained_revision.is_some() {
            "Обновление отклонено; отображается предыдущая версия"
        } else if self.scope == FailureScope::EmbeddedDocument {
            "Вложенный документ не отображён"
        } else {
            "Документ не отображён"
        };
        match self.reason {
            FailureReason::XmlSyntax { row, column } => {
                write!(f, "{subject}: ошибка XML ({row}:{column})")
            }
            FailureReason::Budget(error) => write!(f, "{subject}: {error}"),
            FailureReason::Allocation => write!(f, "{subject}: не удалось выделить память"),
            FailureReason::CapacityOverflow => write!(f, "{subject}: переполнение размера буфера"),
        }
    }
}

impl std::error::Error for DocumentFailure {}

impl From<crate::budget::admission::Error> for FailureReason {
    fn from(error: crate::budget::admission::Error) -> Self {
        match error {
            crate::budget::admission::Error::Budget(error) => Self::Budget(error),
            crate::budget::admission::Error::Allocation => Self::Allocation,
            crate::budget::admission::Error::CapacityOverflow => Self::CapacityOverflow,
        }
    }
}

impl From<crate::budget::admission::Error> for DocumentFailure {
    fn from(error: crate::budget::admission::Error) -> Self {
        Self::new(error.into())
    }
}

impl BuildReport {
    /// Обновление действительно заменило committed revision.
    pub fn changed(self) -> bool {
        matches!(
            self.status,
            RevisionStatus::Complete | RevisionStatus::Degraded
        )
    }
}
