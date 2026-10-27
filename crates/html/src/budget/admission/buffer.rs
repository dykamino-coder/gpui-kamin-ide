//! Резерв живёт вместе с backing storage; обе генерации оплачены при переносе.

use super::{Admission, Error};
use crate::budget::LiveReservation;
use std::{
    mem,
    ops::{Deref, DerefMut},
};

pub(crate) struct Buffer<T> {
    values: Vec<T>,
    lease: Option<LiveReservation>,
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            lease: None,
        }
    }
}

impl<T> Buffer<T> {
    pub(crate) fn reserve(
        &mut self,
        additional: usize,
        admission: &Admission,
    ) -> Result<(), Error> {
        if additional <= self.values.capacity() - self.values.len() {
            return Ok(());
        }
        let capacity = self
            .values
            .len()
            .checked_add(additional)
            .and_then(|n| n.max(4).checked_next_power_of_two())
            .ok_or(Error::CapacityOverflow)?;
        let bytes = capacity
            .checked_mul(mem::size_of::<T>())
            .ok_or(Error::CapacityOverflow)?;
        admission.work(self.values.len())?;
        let lease = admission.storage(bytes)?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation)?;
        next.append(&mut self.values);
        drop(mem::replace(&mut self.values, next));
        self.lease = lease;
        Ok(())
    }

    pub(crate) fn push(&mut self, value: T, admission: &Admission) -> Result<(), Error> {
        self.reserve(1, admission)?;
        self.values.push(value);
        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }
}

impl<T> Deref for Buffer<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.values
    }
}

impl<T> DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.values
    }
}
