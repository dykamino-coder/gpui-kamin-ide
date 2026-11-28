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
