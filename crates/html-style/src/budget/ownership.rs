//! Physical reservation следует за storage, logical admission — за документом.

use super::{BudgetError, BudgetKind, DocumentBudget, Reservation, ResourceGovernor};
use std::sync::Arc;

#[derive(Debug)]
pub enum StorageError<E> {
    Budget(BudgetError),
    Allocation(E),
}

/// Payload уничтожается до reservation; clone Arc и eviction не возвращают квоту.
/// T владеет allocation: независимо клонируемые handles должны сами удерживать
/// SharedStorage, а не выносить из T uncharged Arc на тот же физический буфер.
pub struct SharedStorage<T> {
    value: T,
    kind: BudgetKind,
    amount: u64,
    governor: ResourceGovernor,
    _reservation: Reservation,
}

impl<T> SharedStorage<T> {
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl ResourceGovernor {
    /// Callback использует fallible allocation и учитывает capacity, temporary
    /// buffers и overhead в amount. Err освобождает reservation до возврата.
    pub fn allocate<T, E>(
        &self,
        kind: BudgetKind,
        amount: u64,
        allocate: impl FnOnce() -> Result<T, E>,
    ) -> Result<Arc<SharedStorage<T>>, StorageError<E>> {
        let reservation = self.reserve(kind, amount).map_err(StorageError::Budget)?;
        let value = allocate().map_err(StorageError::Allocation)?;
        Ok(Arc::new(SharedStorage {
            value,
            kind,
            amount,
            governor: self.clone(),
            _reservation: reservation,
        }))
    }
}

/// Logical admission не освобождает physical storage после cache eviction.
pub struct AdmittedStorage<T> {
    storage: Arc<SharedStorage<T>>,
    _logical: Reservation,
    _document: DocumentBudget,
}

impl<T> AdmittedStorage<T> {
    pub fn value(&self) -> &T {
        self.storage.value()
    }
}

impl DocumentBudget {
    pub fn admit<T>(
        &self,
        storage: Arc<SharedStorage<T>>,
    ) -> Result<AdmittedStorage<T>, BudgetError> {
        if !Arc::ptr_eq(&self.governor().ledger, &storage.governor.ledger) {
            return Err(BudgetError::ForeignGovernor);
        }
        let logical = self.logical(storage.kind, storage.amount)?;
        Ok(AdmittedStorage {
            storage,
            _logical: logical,
            _document: self.clone(),
        })
    }
}
