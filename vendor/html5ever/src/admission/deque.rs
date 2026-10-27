//! A parser work queue owns its reservation until its backing allocation dies.

use super::storage::{Lease, ReserveError};
use std::{
    collections::VecDeque,
    mem,
    ops::{Deref, DerefMut},
};

pub struct StorageDeque<T> {
    values: VecDeque<T>,
    lease: Lease,
}
impl<T> StorageDeque<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn try_reserve_with<E>(
        &mut self,
        additional: usize,
        admit: impl FnOnce(usize) -> Result<Lease, E>,
    ) -> Result<(), ReserveError<E>> {
        if additional <= self.values.capacity() - self.values.len() {
            return Ok(());
        }
        let required = self
            .values
            .len()
            .checked_add(additional)
            .ok_or(ReserveError::Overflow)?;
        let capacity = required
            .max(4)
            .checked_next_power_of_two()
            .ok_or(ReserveError::Overflow)?;
        let bytes = capacity
            .checked_mul(mem::size_of::<T>())
            .ok_or(ReserveError::Overflow)?;
        let lease = admit(bytes).map_err(ReserveError::Admission)?;
        let mut next = VecDeque::new();
        next.try_reserve_exact(capacity)
            .map_err(ReserveError::Allocation)?;
        next.append(&mut self.values);
        let previous = mem::replace(&mut self.values, next);
        drop(previous);
        self.lease = lease;
        Ok(())
    }
}
impl<T> Default for StorageDeque<T> {
    fn default() -> Self {
        Self {
            values: VecDeque::new(),
            lease: None,
        }
    }
}
impl<T> Deref for StorageDeque<T> {
    type Target = VecDeque<T>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl<T> DerefMut for StorageDeque<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}
