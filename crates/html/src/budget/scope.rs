//! Владельцы ledger: документ переживает update, work принадлежит попытке.

use super::ledger::Ledger;
use super::{
    BudgetClass, BudgetError, BudgetKind, BudgetScope, DocumentLimits, ProcessLimits, Reservation,
};
use std::sync::{Arc, OnceLock};

/// Равенство и hash — идентичность host-домена учёта, а не равенство квот.
#[derive(Clone)]
pub struct ResourceGovernor {
    pub(super) ledger: Arc<Ledger>,
}

impl PartialEq for ResourceGovernor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ledger, &other.ledger)
    }
}
impl Eq for ResourceGovernor {}
impl std::hash::Hash for ResourceGovernor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.ledger).hash(state);
    }
}

impl Default for ResourceGovernor {
    fn default() -> Self {
        Self::new(ProcessLimits::default())
    }
}

impl ResourceGovernor {
    pub fn new(limits: ProcessLimits) -> Self {
        Self {
            ledger: Ledger::new(BudgetScope::Process, limits.caps),
        }
    }
    /// Physical storage общего cache/pool. Резервировать до allocation.
    pub fn reserve(&self, kind: BudgetKind, amount: u64) -> Result<Reservation, BudgetError> {
        self.ledger.reserve(kind, amount)
    }
    pub fn used(&self, kind: BudgetKind) -> u64 {
        self.ledger.used(kind)
    }
}

struct DocumentOwner {
    ledger: Arc<Ledger>,
    governor: ResourceGovernor,
    limits: DocumentLimits,
    _slot: Reservation,
    _document_slot: Reservation,
}

/// Clone/view использует тот же ledger и тот же process slot.
#[derive(Clone)]
pub struct DocumentBudget {
    owner: Arc<DocumentOwner>,
}

/// Квоты одной allocation: логическая документа и physical хоста.
#[must_use = "retain alongside the allocation until its last owner drops"]
pub struct LiveReservation {
    _document: Reservation,
    _physical: Reservation,
    _owner: DocumentBudget,
}

impl DocumentBudget {
    pub fn new(limits: DocumentLimits, governor: ResourceGovernor) -> Result<Self, BudgetError> {
        let ledger = Ledger::new(BudgetScope::Document, limits.caps);
        let document_slot = ledger.reserve(BudgetKind::Documents, 1)?;
        let slot = governor.reserve(BudgetKind::Documents, 1)?;
        Ok(Self {
            owner: Arc::new(DocumentOwner {
                ledger,
                governor,
                limits,
                _slot: slot,
                _document_slot: document_slot,
            }),
        })
    }
    pub fn limits(&self) -> &DocumentLimits {
        &self.owner.limits
    }
    pub fn governor(&self) -> &ResourceGovernor {
        &self.owner.governor
    }
    pub fn used(&self, kind: BudgetKind) -> u64 {
        self.owner.ledger.used(kind)
    }
    pub fn revision(&self) -> RevisionBudget {
        RevisionBudget {
            document: self.clone(),
            ledger: Ledger::new(BudgetScope::Revision, self.owner.limits.caps),
            exhausted: Arc::default(),
        }
    }
    pub fn reserve(&self, kind: BudgetKind, amount: u64) -> Result<LiveReservation, BudgetError> {
        let local = self.logical(kind, amount)?;
        let physical = self.governor().reserve(kind, amount)?;
        Ok(LiveReservation {
            _document: local,
            _physical: physical,
            _owner: self.clone(),
        })
    }
    pub(super) fn logical(
        &self,
        kind: BudgetKind,
        amount: u64,
    ) -> Result<Reservation, BudgetError> {
        self.owner.ledger.reserve(kind, amount)
    }
}

/// Subjobs получают clone этой ревизии; retry внутри неё не возвращает work.
/// Любой отказ её ledger терминален для ревизии (ADR2 §4): первая причина
/// сохраняется для build/update, даже если операция сама вернула локальный fallback.
#[derive(Clone)]
pub struct RevisionBudget {
    document: DocumentBudget,
    ledger: Arc<Ledger>,
    exhausted: Arc<OnceLock<BudgetError>>,
}

impl RevisionBudget {
    pub fn document(&self) -> &DocumentBudget {
        &self.document
    }
    pub fn used(&self, kind: BudgetKind) -> u64 {
        self.ledger.used(kind)
    }
    pub fn consume(&self, kind: BudgetKind, amount: u64) -> Result<(), BudgetError> {
        Ledger::check_class(kind, BudgetClass::Monotonic)
            .and_then(|()| self.ledger.charge(kind, amount))
            .inspect_err(|error| self.terminate(*error))
    }
    /// Structural maxima — не refundable work и не сумма повторных проходов.
    pub fn check_maximum(&self, kind: BudgetKind, total: u64) -> Result<(), BudgetError> {
        Ledger::check_class(kind, BudgetClass::Structural)
            .and_then(|()| self.ledger.check(kind, total))
            .inspect_err(|error| self.terminate(*error))
    }
    /// Ревизия состояния без Document (фикстуры, вложенные состояния до B3):
    /// собственный host-домен со стандартным конечным профилем, не безлимитный sentinel.
    pub(crate) fn standalone() -> Self {
        DocumentBudget::new(DocumentLimits::default(), ResourceGovernor::default())
            .expect("a fresh governor admits its first document")
            .revision()
    }
    /// Первая терминальная причина ревизии; поздние отказы её не заменяют.
    pub fn exhausted(&self) -> Option<BudgetError> {
        self.exhausted.get().copied()
    }
    fn terminate(&self, error: BudgetError) {
        let _ = self.exhausted.set(error);
    }
    pub(crate) fn source(&self, html: &str, theme: &str) -> Result<(), BudgetError> {
        self.consume(BudgetKind::SourceBytes, html.len() as u64)?;
        self.consume(BudgetKind::SourceBytes, theme.len() as u64)
    }
}
