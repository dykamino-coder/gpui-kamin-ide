//! Строка раннего прохода с fallible ростом и резервом до освобождения байтов.

use super::{Admission, Error};
use crate::budget::LiveReservation;
use std::{fmt, mem, ops::Deref};

#[derive(Default)]
pub(crate) struct Text {
    value: String,
    lease: Option<LiveReservation>,
}

impl Text {
    pub(crate) fn from_str(value: &str, admission: &Admission) -> Result<Self, Error> {
        let mut out = Self::default();
        out.push_str(value, admission)?;
        Ok(out)
    }

    pub(crate) fn push_str(&mut self, value: &str, admission: &Admission) -> Result<(), Error> {
        if value.len() > self.value.capacity() - self.value.len() {
            let capacity = self
                .value
                .len()
                .checked_add(value.len())
                .and_then(|n| n.max(8).checked_next_power_of_two())
                .ok_or(Error::CapacityOverflow)?;
            admission.work(self.value.len())?;
            let lease = admission.storage(capacity)?;
            let mut next = String::new();
            next.try_reserve_exact(capacity)
                .map_err(|_| Error::Allocation)?;
            next.push_str(&self.value);
            drop(mem::replace(&mut self.value, next));
            self.lease = lease;
        }
        admission.work(value.len())?;
        self.value.push_str(value);
        Ok(())
    }
}

impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
