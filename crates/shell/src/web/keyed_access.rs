//! AcquireSync возвращает также положительные WAIT_*; SUCCEEDED/Result<()> не
//! означает владение. Только S_OK разрешает copy и ReleaseSync.

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Access {
    Copied,
    Retry,
    Recreate,
}

pub(super) fn with_access(
    status: Option<i32>,
    copy: impl FnOnce(),
    release: impl FnOnce(),
) -> Access {
    match status {
        Some(0) => {
            copy();
            release();
            Access::Copied
        }
        Some(128) | None => Access::Recreate,
        _ => Access::Retry,
    }
}

#[derive(Default)]
pub(super) struct RetryBudget(u8);
impl RetryBudget {
    pub(super) fn take(&mut self) -> bool {
        if self.0 >= 3 {
            return false;
        }
        self.0 += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn exact_status_controls_copy_and_release() {
        for (status, expected) in [
            (Some(0), Access::Copied),
            (Some(258), Access::Retry),
            (Some(128), Access::Recreate),
            (Some(0x80004005u32 as i32), Access::Retry),
            (None, Access::Recreate),
            (Some(1), Access::Retry),
        ] {
            let copied = Cell::new(0);
            let released = Cell::new(0);
            let actual = with_access(
                status,
                || copied.set(copied.get() + 1),
                || released.set(released.get() + 1),
            );
            assert_eq!(actual, expected);
            assert_eq!(copied.get(), i32::from(status == Some(0)));
            assert_eq!(released.get(), copied.get());
        }
    }
    #[test]
    fn repeated_wait_failure_cannot_spin_without_new_producer_frame() {
        let mut budget = RetryBudget::default();
        assert_eq!((0..100).filter(|_| budget.take()).count(), 3);
        let mut new_frame = RetryBudget::default();
        assert!(new_frame.take());
    }
}
