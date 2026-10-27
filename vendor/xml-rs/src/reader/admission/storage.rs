//! Fallible owners used by the canonical core and bounded events.

use super::{control::Control, Lease};
use crate::reader::Error;
use std::{
    fmt, mem,
    ops::{Deref, DerefMut},
};

/// A vector whose reservation follows the backing allocation through moves.
pub struct Storage<T> {
    values: Vec<T>,
    lease: Lease,
}
impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Storage<T> {
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            lease: None,
        }
    }
    pub(crate) fn reserve(&mut self, additional: usize, control: &Control) -> Result<(), Error> {
        if additional <= self.values.capacity() - self.values.len() {
            return Ok(());
        }
        let capacity = self
            .values
            .len()
            .checked_add(additional)
            .and_then(|n| n.max(4).checked_next_power_of_two())
            .ok_or_else(|| control.overflow())?;
        let bytes = capacity
            .checked_mul(mem::size_of::<T>())
            .ok_or_else(|| control.overflow())?;
        control.work(self.values.len())?;
        let lease = control.storage(bytes)?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| control.allocation())?;
        next.append(&mut self.values);
        drop(mem::replace(&mut self.values, next));
        self.lease = lease;
        Ok(())
    }
    pub(crate) fn push(&mut self, value: T, control: &Control) -> Result<(), Error> {
        self.reserve(1, control)?;
        self.values.push(value);
        Ok(())
    }
    pub(crate) fn insert(
        &mut self,
        index: usize,
        value: T,
        control: &Control,
    ) -> Result<(), Error> {
        control.work(self.values.len() - index)?;
        self.reserve(1, control)?;
        self.values.insert(index, value);
        Ok(())
    }
    pub(crate) fn into_legacy(self) -> Vec<T> {
        self.values
    }
}

impl<T> Deref for Storage<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.values
    }
}
impl<T> DerefMut for Storage<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.values
    }
}
impl<T: fmt::Debug> fmt::Debug for Storage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.values.fmt(f)
    }
}
impl<T> Storage<T> {
    pub(crate) fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }
    pub(crate) fn remove(&mut self, index: usize) -> T {
        self.values.remove(index)
    }
    pub(crate) fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }
}

/// UTF-8 owned storage. Mutation requires admission before allocating or copying.
#[derive(Default)]
pub struct Text {
    value: String,
    lease: Lease,
}

impl Text {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    fn reserve(&mut self, additional: usize, control: &Control) -> Result<(), Error> {
        if additional <= self.value.capacity() - self.value.len() {
            return Ok(());
        }
        let capacity = self
            .value
            .len()
            .checked_add(additional)
            .and_then(|n| n.max(8).checked_next_power_of_two())
            .ok_or_else(|| control.overflow())?;
        control.work(self.value.len())?;
        let lease = control.storage(capacity)?;
        let mut next = String::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| control.allocation())?;
        next.push_str(&self.value);
        drop(mem::replace(&mut self.value, next));
        self.lease = lease;
        Ok(())
    }
    pub(crate) fn from_str(value: &str, control: &Control) -> Result<Self, Error> {
        let mut text = Self::new();
        text.push_str(value, control)?;
        Ok(text)
    }
    pub(crate) fn push(&mut self, value: char, control: &Control) -> Result<(), Error> {
        self.reserve(value.len_utf8(), control)?;
        control.work(value.len_utf8())?;
        self.value.push(value);
        Ok(())
    }
    pub(crate) fn push_str(&mut self, value: &str, control: &Control) -> Result<(), Error> {
        self.reserve(value.len(), control)?;
        control.work(value.len())?;
        self.value.push_str(value);
        Ok(())
    }
    pub(crate) fn try_clone(&self, control: &Control) -> Result<Self, Error> {
        Self::from_str(self, control)
    }
    pub(crate) fn clear(&mut self) {
        self.value.clear();
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
    pub(crate) fn into_legacy(self) -> String {
        self.value
    }
}
impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Text {}
impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}
impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.value == *other
    }
}
// Only the legacy diagnostic closure converts to Box; bounded errors never run it.
impl From<Text> for Box<str> {
    fn from(value: Text) -> Self {
        value.value.into_boxed_str()
    }
}
