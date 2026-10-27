//! Конечные квоты ADR2: work ревизии, живая память документа и storage хоста.
//!
//! Ledger учитывает только явно подключённые операции, не RSS. B3 подключает
//! parser allocations, B4 — resource/decode/var; лимит проверяется до операции.

pub(crate) mod admission;
mod ledger;
mod ownership;
#[cfg(test)]
mod ownership_tests;
mod profile;
mod scope;
#[cfg(test)]
mod tests;
pub use ledger::{BudgetError, BudgetExceeded, BudgetScope, Reservation};
pub use ownership::{AdmittedStorage, SharedStorage, StorageError};
pub use profile::{
    BudgetClass, BudgetKind, BudgetUnit, DocumentLimits, InvalidLimit, ProcessLimits,
};
pub use scope::{DocumentBudget, LiveReservation, ResourceGovernor, RevisionBudget};
