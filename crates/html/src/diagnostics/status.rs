//! Постоянный статус представления для host UI (ADR2 §5): что показано и почему.
//! Только данные и текст статуса; вид индикатора выбирает host.

use crate::dom::outcome::{DocumentFailure, TraversalLimit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayStatus {
    /// Ревизия показана полностью.
    Complete,
    /// Показана со штатным fallback: предел представления или отказ вложенного
    /// документа. `problems` — число сгруппированных записей диагностики.
    Degraded {
        first_limit: Option<TraversalLimit>,
        embedded_failure: Option<DocumentFailure>,
        problems: usize,
        suppressed: u64,
    },
    /// Update не применён; показана прежняя committed revision.
    Stale(DocumentFailure),
    /// Первая загрузка отклонена: документа нет, показывается только причина.
    Failed(DocumentFailure),
}

impl DisplayStatus {
    /// Текст постоянной строки статуса; `None` для полной ревизии.
    pub fn message(&self) -> Option<String> {
        match self {
            Self::Complete => None,
            Self::Stale(failure) | Self::Failed(failure) => Some(failure.to_string()),
            Self::Degraded {
                first_limit,
                embedded_failure,
                problems,
                suppressed,
            } => {
                let first = first_limit
                    .map(|limit| limit.to_string())
                    .or_else(|| embedded_failure.map(|failure| failure.to_string()))
                    .unwrap_or_else(|| "Документ отображён с ограничениями".to_owned());
                let others = problems.saturating_sub(1) as u64 + suppressed;
                Some(if others == 0 {
                    first
                } else {
                    format!("{first}. Ещё проблем: {others}; подробности в панели диагностики")
                })
            }
        }
    }

    /// Документ не заменил прошлый или не отображён вовсе.
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Stale(_) | Self::Failed(_))
    }
}
