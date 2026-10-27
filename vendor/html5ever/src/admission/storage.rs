//! Parser vectors keep a reservation with the allocation, including moves.

use std::{
    any::Any,
    collections::TryReserveError,
    mem,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub type Lease = Option<Arc<dyn Any + Send + Sync>>;

#[derive(Debug)]
pub enum ReserveError<E> {
    Admission(E),
    Allocation(TryReserveError),
    Overflow,
}

/// The owner follows the Vec through tokens, formatting entries and worklists.
pub struct Storage<T> {
    values: Vec<T>,
    lease: Lease,
}

impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            lease: None,
        }
    }
}
impl<T> From<Vec<T>> for Storage<T> {
    fn from(values: Vec<T>) -> Self {
        Self {
            values,
            lease: None,
        }
    }
}
impl<T> Deref for Storage<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.values
    }
}
impl<T> DerefMut for Storage<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.values
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for Storage<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.values.fmt(fmt)
    }
}
impl<T: PartialEq> PartialEq for Storage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}
impl<T: Eq> Eq for Storage<T> {}
impl<T: Clone> Clone for Storage<T> {
    // The legacy Token API is infallible. Canonical bounded parsing uses its
    // admitted clone_attrs path instead of this convenience implementation.
    fn clone(&self) -> Self {
        self.values.clone().into()
    }
}

impl<T> Storage<T> {
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
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(ReserveError::Allocation)?;
        next.append(&mut self.values);
        let previous = mem::replace(&mut self.values, next);
        drop(previous);
        self.lease = lease;
        Ok(())
    }

    /// The bounded sink must copy into its own admitted storage before this
    /// callback returns. The source reservation covers the entire callback.
    pub(crate) fn with_vec<R>(mut self, callback: impl FnOnce(Vec<T>) -> R) -> R {
        callback(mem::take(&mut self.values))
    }
}

pub struct IntoIter<T> {
    values: std::vec::IntoIter<T>,
    _lease: Lease,
}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.values.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}
impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.values.next_back()
    }
}
impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> IntoIterator for Storage<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            values: self.values.into_iter(),
            _lease: self.lease,
        }
    }
}
impl<'a, T> IntoIterator for &'a Storage<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}
impl<'a, T> IntoIterator for &'a mut Storage<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter_mut()
    }
}
