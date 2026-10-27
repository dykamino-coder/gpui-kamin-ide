//! Допуск одного чтения ресурса в work ревизии (ADR2 B4): попытка загрузки — запрос,
//! прочитанные или декодированные из `data:` байты списываются ДО роста буфера. Предел
//! одного ресурса (S6, `limits`) остаётся независимым от суммы документа. Отмена задания —
//! отдельный исход, проверяемый между порциями чтения.

use super::LoadError;
use crate::budget::{BudgetKind, RevisionBudget};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Отмена заданий одного поколения: устаревший документ или закрытое хранилище.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Наблюдение предела или отмены загрузки; обычные ошибки чтения — не диагностика пределов.
pub(crate) fn diagnostic(error: &LoadError) -> Option<crate::diagnostics::Diagnostic> {
    use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};
    Some(match *error {
        LoadError::TooLarge { limit } => Diagnostic::observed(
            Code::ResourceLimit,
            Severity::Warning,
            Phase::Resource,
            Some(Measure::Ceiling {
                limit: limit as u64,
            }),
        ),
        LoadError::Budget(error) => Diagnostic::budget(Phase::Resource, error),
        LoadError::Cancelled => {
            Diagnostic::observed(Code::Cancelled, Severity::Info, Phase::Resource, None)
        }
        _ => return None,
    })
}

/// Загрузка с допуском: запрос ревизии списывается до обращения к бэкенду.
pub(crate) fn fetch(
    loader: &dyn super::ResourceLoader,
    url: &super::ResourceUrl,
    revision: &RevisionBudget,
    cancel: &Cancellation,
) -> Result<super::Loaded, LoadError> {
    loader.load(url, &Ingress::request(revision, cancel)?)
}

/// Учтённая попытка загрузки: списанный запрос ревизии и её отмена.
pub(crate) struct Ingress<'a> {
    revision: &'a RevisionBudget,
    cancel: &'a Cancellation,
}

impl<'a> Ingress<'a> {
    /// Списать запрос до обращения к бэкенду; повтор и revalidation — тоже запросы.
    pub(crate) fn request(
        revision: &'a RevisionBudget,
        cancel: &'a Cancellation,
    ) -> Result<Self, LoadError> {
        let ingress = Self { revision, cancel };
        ingress.check()?;
        revision
            .consume(BudgetKind::ResourceRequests, 1)
            .map_err(LoadError::Budget)?;
        Ok(ingress)
    }

    /// Допустить `bytes` новых байт ресурса до их копирования в буфер.
    pub(crate) fn admit(&self, bytes: usize) -> Result<(), LoadError> {
        self.check()?;
        self.revision
            .consume(BudgetKind::ResourceBytes, bytes as u64)
            .map_err(LoadError::Budget)
    }

    /// Отменённое задание прекращает работу на ближайшей границе.
    pub(crate) fn check(&self) -> Result<(), LoadError> {
        if self.cancel.is_cancelled() {
            Err(LoadError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetError, DocumentBudget, DocumentLimits, ResourceGovernor};
    use crate::resources::loader::{ResourceUrl, SyncLoader};

    fn revision(kind: BudgetKind, limit: u64) -> RevisionBudget {
        let limits = DocumentLimits::default().lower(kind, limit).unwrap();
        DocumentBudget::new(limits, ResourceGovernor::default())
            .unwrap()
            .revision()
    }

    fn load(url: &str, revision: &RevisionBudget) -> Result<usize, LoadError> {
        let (url, _) = ResourceUrl::resolve(url, None);
        let loaded = fetch(&SyncLoader, &url, revision, &Cancellation::default())?;
        Ok(loaded.bytes.len())
    }

    fn exceeded(error: LoadError, kind: BudgetKind) -> bool {
        matches!(error, LoadError::Budget(BudgetError::Exceeded(e)) if e.kind == kind)
    }

    #[test]
    fn decoded_data_bytes_boundary_charges_the_revision() {
        // Три байта после декодирования base64, а не длина закодированного текста.
        let url = "data:text/plain;base64,YWJj";
        for (limit, ok) in [(2, false), (3, true), (4, true)] {
            let budget = revision(BudgetKind::ResourceBytes, limit);
            match load(url, &budget) {
                Ok(len) => assert!(ok && len == 3),
                Err(error) => assert!(!ok && exceeded(error, BudgetKind::ResourceBytes)),
            }
            let used = budget.used(BudgetKind::ResourceBytes);
            assert_eq!(used, if ok { 3 } else { 0 });
            assert_eq!(budget.exhausted().is_some(), !ok);
        }
    }

    #[test]
    fn many_small_resources_share_the_aggregate_and_requests() {
        let url = "data:,abcd";
        let budget = revision(BudgetKind::ResourceBytes, 10);
        assert_eq!(load(url, &budget), Ok(4));
        assert_eq!(load(url, &budget), Ok(4));
        assert!(exceeded(
            load(url, &budget).unwrap_err(),
            BudgetKind::ResourceBytes
        ));
        assert_eq!(budget.used(BudgetKind::ResourceBytes), 8);
        assert_eq!(budget.used(BudgetKind::ResourceRequests), 3);
        for (limit, ok) in [(1, 1), (2, 2), (3, 3)] {
            let budget = revision(BudgetKind::ResourceRequests, limit);
            let done = (0..3).filter(|_| load(url, &budget).is_ok()).count();
            assert_eq!(done, ok.min(3));
        }
        let budget = revision(BudgetKind::ResourceRequests, 2);
        // Неизвестная схема — тоже запрос: третий отклоняет уже ревизия.
        assert!(load("http://x/a.png", &budget).is_err());
        assert!(load("http://x/a.png", &budget).is_err());
        let third = load("http://x/a.png", &budget).unwrap_err();
        assert!(exceeded(third, BudgetKind::ResourceRequests));
    }

    #[test]
    fn cancelled_ingress_neither_requests_nor_reads() {
        let budget = revision(BudgetKind::ResourceBytes, 64);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert_eq!(
            Ingress::request(&budget, &cancel).err(),
            Some(LoadError::Cancelled)
        );
        assert_eq!(budget.used(BudgetKind::ResourceRequests), 0);
        assert!(budget.exhausted().is_none(), "cancellation is not a limit");
    }
}
