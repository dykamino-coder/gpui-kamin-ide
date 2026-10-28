//! Checked atomic counters. Только live reservation допускает возврат.

use super::{BudgetClass, BudgetKind, BudgetUnit};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetScope {
    Revision,
    Document,
    Process,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetExceeded {
    pub scope: BudgetScope,
    pub kind: BudgetKind,
    pub unit: BudgetUnit,
    pub limit: u64,
    pub consumed: u64,
    pub increment: u64,
    pub overflow: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetError {
    Exceeded(BudgetExceeded),
    WrongClass {
        kind: BudgetKind,
        expected: BudgetClass,
    },
    ForeignGovernor,
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exceeded(e) => write!(
                f,
                "{:?} {:?}: {} + {} > {} {:?} (overflow={})",
                e.scope, e.kind, e.consumed, e.increment, e.limit, e.unit, e.overflow
            ),
            Self::WrongClass { kind, expected } => {
                write!(f, "{kind:?}: expected {expected:?} counter")
            }
            Self::ForeignGovernor => write!(f, "storage belongs to another resource governor"),
        }
    }
}
impl std::error::Error for BudgetError {}

pub(super) struct Ledger {
    pub scope: BudgetScope,
    pub caps: [u64; BudgetKind::COUNT],
    used: [AtomicU64; BudgetKind::COUNT],
}

impl Ledger {
    pub fn new(scope: BudgetScope, caps: [u64; BudgetKind::COUNT]) -> Arc<Self> {
        Arc::new(Self {
            scope,
            caps,
            used: std::array::from_fn(|_| AtomicU64::new(0)),
        })
    }
    pub fn used(&self, kind: BudgetKind) -> u64 {
        self.used[kind as usize].load(Ordering::Relaxed)
    }
    pub fn check_class(kind: BudgetKind, expected: BudgetClass) -> Result<(), BudgetError> {
        if kind.class() != expected {
            Err(BudgetError::WrongClass { kind, expected })
        } else {
            Ok(())
        }
    }
    fn excess(&self, kind: BudgetKind, used: u64, amount: u64) -> BudgetError {
        BudgetError::Exceeded(BudgetExceeded {
            scope: self.scope,
            kind,
            unit: kind.unit(),
            limit: self.caps[kind as usize],
            consumed: used,
            increment: amount,
            overflow: used.checked_add(amount).is_none(),
        })
    }
    pub fn check(&self, kind: BudgetKind, amount: u64) -> Result<(), BudgetError> {
        if amount > self.caps[kind as usize] {
            let used = self.used(kind);
            return Err(self.excess(kind, used, amount - used));
        }
        self.used[kind as usize].fetch_max(amount, Ordering::Relaxed);
        Ok(())
    }
    pub fn charge(&self, kind: BudgetKind, amount: u64) -> Result<(), BudgetError> {
        self.used[kind as usize]
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(amount)
                    .filter(|&sum| sum <= self.caps[kind as usize])
            })
            .map(|_| ())
            .map_err(|used| self.excess(kind, used, amount))
    }
    pub fn reserve(
        self: &Arc<Self>,
        kind: BudgetKind,
        amount: u64,
    ) -> Result<Reservation, BudgetError> {
        Self::check_class(kind, BudgetClass::Live)?;
        self.charge(kind, amount)?;
        Ok(Reservation {
            ledger: self.clone(),
            kind,
            amount,
        })
    }
}

/// Move-only RAII token; держать рядом с allocation до фактического drop.
#[must_use = "dropping the token releases the live allocation's quota"]
pub struct Reservation {
    ledger: Arc<Ledger>,
    kind: BudgetKind,
    amount: u64,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.ledger.used[self.kind as usize].fetch_sub(self.amount, Ordering::Relaxed);
    }
}
