//! Компактный исход ревизии: отказ либо ограниченное представление целого DOM.

/// Причина отказа без исходной разметки и адресов ресурсов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    XmlSyntax { row: u32, column: u32 },
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
        "KAMINIDE_LIMIT_TRAVERSAL_DEPTH"
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

/// B1 хранит первый предел; очередь последующих диагностик относится к B5.
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
        match self.reason {
            FailureReason::XmlSyntax { .. } => "KAMINIDE_XML_SYNTAX",
        }
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
        }
    }
}

impl std::error::Error for DocumentFailure {}

impl BuildReport {
    /// Обновление действительно заменило committed revision.
    pub fn changed(self) -> bool {
        matches!(
            self.status,
            RevisionStatus::Complete | RevisionStatus::Degraded
        )
    }
}
