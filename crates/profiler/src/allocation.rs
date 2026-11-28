//! Global opt-in allocation hook. Buffers exist only during a session; the hook
//! captures raw addresses and never symbolizes, serializes, writes files or UI.

use kamin_heap::live::{Ledger, Stack};
use std::{
    cell::Cell,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) static ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static LEDGER: Mutex<Option<Probe>> = Mutex::new(None);
mod sample;

#[doc(hidden)]
pub struct Probe {
    pub(crate) ledger: Ledger,
    pub(crate) session: u64,
    pub(crate) mean: usize,
    pub(crate) recording: bool,
}

impl Probe {
    pub fn allocate(&mut self, address: usize, bytes: usize) {
        if self.recording
            && address != 0
            && bytes != 0
            && sample::selected(self.session, bytes, self.mean)
        {
            self.ledger.allocate(address, bytes, stack());
        }
    }
    pub fn free(&mut self, address: usize) {
        self.ledger.free(address);
    }
    pub fn reallocate(&mut self, old: usize, next: usize, bytes: usize) {
        if next != 0 {
            self.free(old);
            self.allocate(next, bytes);
        }
    }
}

thread_local! { static ENTERED: Cell<bool> = const { Cell::new(false) }; }

pub(crate) struct Suppress;

impl Suppress {
    pub(crate) fn enter() -> Option<Self> {
        ENTERED
            .try_with(|entered| {
                if entered.replace(true) {
                    None
                } else {
                    Some(Self)
                }
            })
            .ok()
            .flatten()
    }
}

impl Drop for Suppress {
    fn drop(&mut self) {
        let _ = ENTERED.try_with(|entered| entered.set(false));
    }
}

/// Adapter boundary: the allocator operation must still run when disabled or
/// reentrant. For realloc/dealloc the caller performs the underlying operation
/// inside this closure, preventing a snapshot from reading a freed tracked block.
#[doc(hidden)]
#[inline]
pub fn with_hook<R>(operation: impl FnOnce(Option<&mut Probe>) -> R) -> R {
    if !ACTIVE.load(Ordering::Relaxed) {
        return operation(None);
    }
    let Some(_entered) = Suppress::enter() else {
        return operation(None);
    };
    let mut ledger = LEDGER.lock().unwrap_or_else(|error| error.into_inner());
    operation(ledger.as_mut())
}

/// Stack capture belongs inside `with_hook` and only runs for an active ledger.
#[doc(hidden)]
pub fn stack() -> Stack {
    #[cfg(windows)]
    {
        let mut stack = Stack::default();
        let mut frames = [std::ptr::null_mut(); kamin_heap::live::STACK_DEPTH];
        // Stack-local correctly sized output, no symbol lookup or heap allocation.
        stack.depth = unsafe {
            windows::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
                2,
                &mut frames,
                None,
            )
        } as u8;
        stack.addresses = frames.map(|address| address as usize);
        stack
    }
    #[cfg(not(windows))]
    {
        Stack::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppression_is_reentrant_and_releases_on_unwind() {
        let first = Suppress::enter().unwrap();
        assert!(Suppress::enter().is_none());
        drop(first);
        assert!(Suppress::enter().is_some());
    }
}
