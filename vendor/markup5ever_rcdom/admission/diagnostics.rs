//! Diagnostic text is admitted before formatting and retained with its owner.

use super::{reserve, Admission, Lease};
use crate::RcDom;
use std::{borrow::Cow, fmt};

struct Count<'a, A: Admission> {
    policy: &'a A,
    bytes: usize,
    error: Option<A::Error>,
}

impl<A: Admission> fmt::Write for Count<'_, A> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let result = self.policy.work(text.len()).and_then(|()| {
            self.bytes = self
                .bytes
                .checked_add(text.len())
                .ok_or_else(|| self.policy.overflow())?;
            Ok(())
        });
        result.map_err(|error| {
            self.error = Some(error);
            fmt::Error
        })
    }
}

impl RcDom {
    fn push_error<A: Admission>(
        &self,
        message: Cow<'static, str>,
        lease: Lease,
        policy: &A,
    ) -> Result<(), A::Error> {
        reserve(
            &mut self.errors.borrow_mut(),
            &mut self.errors_lease.borrow_mut(),
            1,
            policy,
        )?;
        if lease.is_some() {
            self.errors_owned.borrow_mut().push(lease, policy)?;
        }
        self.errors.borrow_mut().push(message);
        Ok(())
    }

    pub fn try_parse_error<A: Admission>(
        &self,
        message: Cow<'static, str>,
        policy: &A,
    ) -> Result<(), A::Error> {
        let lease = match &message {
            Cow::Owned(text) => policy.storage(text.capacity())?,
            Cow::Borrowed(_) => None,
        };
        self.push_error(message, lease, policy)
    }

    pub fn try_parse_error_fmt<A: Admission>(
        &self,
        message: fmt::Arguments<'_>,
        policy: &A,
    ) -> Result<(), A::Error> {
        let mut count = Count {
            policy,
            bytes: 0,
            error: None,
        };
        if fmt::write(&mut count, message).is_err() {
            return Err(count.error.unwrap_or_else(|| policy.overflow()));
        }
        // Both formatting passes are admitted before the allocating pass.
        policy.work(count.bytes)?;
        let lease = policy.storage(count.bytes)?;
        let mut text = String::new();
        text.try_reserve_exact(count.bytes)
            .map_err(|error| policy.allocation(error))?;
        fmt::write(&mut text, message).map_err(|_| policy.overflow())?;
        self.push_error(Cow::Owned(text), lease, policy)
    }
}
