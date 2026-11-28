//! Feature-gated adapter; without `profiler` no wrapper or hook is compiled.

use kamin_profiler::allocation::with_hook;
use std::alloc::{GlobalAlloc, Layout};

/// Wrap any application's existing allocator without changing its policy.
pub struct Profiled<A>(pub A);

// SAFETY: all layouts, return values and alignment guarantees are forwarded to A.
// The ledger only records successful non-null allocations. Deallocation/realloc
// stays under its lock, so an address cannot be reused ahead of its free record.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Profiled<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.0.alloc(layout) };
        if !pointer.is_null() {
            with_hook(|ledger| {
                if let Some(ledger) = ledger {
                    ledger.allocate(pointer as usize, layout.size());
                }
            });
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.0.alloc_zeroed(layout) };
        if !pointer.is_null() {
            with_hook(|ledger| {
                if let Some(ledger) = ledger {
                    ledger.allocate(pointer as usize, layout.size());
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        with_hook(|ledger| {
            if let Some(ledger) = ledger {
                ledger.free(pointer as usize);
            }
            unsafe { self.0.dealloc(pointer, layout) };
        });
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        with_hook(|ledger| {
            let next = unsafe { self.0.realloc(pointer, layout, size) };
            if let Some(ledger) = ledger {
                if !next.is_null() {
                    ledger.reallocate(pointer as usize, next as usize, size);
                }
            }
            next
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kamin_profiler::session::{Config, Session};
    use std::alloc::System;

    struct FailedRealloc;

    unsafe impl GlobalAlloc for FailedRealloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) }
        }
        unsafe fn realloc(&self, _: *mut u8, _: Layout, _: usize) -> *mut u8 {
            std::ptr::null_mut()
        }
    }

    #[test]
    fn forwards_zeroing_alignment_and_failed_realloc_without_losing_old_block() {
        let mut session = Session::start(Config {
            max_blocks: 8,
            max_events: 20,
            sampling_mean: 0,
        })
        .unwrap();
        let allocator = Profiled(FailedRealloc);
        let layout = Layout::from_size_align(31, 64).unwrap();
        unsafe {
            let pointer = allocator.alloc_zeroed(layout);
            assert!(!pointer.is_null());
            assert_eq!(pointer as usize % 64, 0);
            assert!(
                std::slice::from_raw_parts(pointer, layout.size())
                    .iter()
                    .all(|&byte| byte == 0)
            );
            let before = session.snapshot();
            assert_eq!(before.live.len(), 1);
            assert_eq!(before.totals.live_bytes, 31);
            *pointer = 42;
            assert!(allocator.realloc(pointer, layout, 100).is_null());
            let after = session.snapshot();
            assert_eq!(after.live, before.live);
            assert_eq!(*pointer, 42);
            allocator.dealloc(pointer, layout);
            assert_eq!(session.snapshot().totals.live_bytes, 0);
        }
        drop(session);
        let layout = Layout::from_size_align(8, 8).unwrap();
        unsafe {
            let pointer = allocator.alloc(layout);
            assert!(!pointer.is_null());
            allocator.dealloc(pointer, layout);
        }
    }
}
