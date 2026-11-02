//! Диагностический счётчик запросов памяти процесса; освобождения не вычитаются.
//! Обёртка сохраняет контракт MiMalloc и включается только вместе с work-counters.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

pub(super) struct Allocator;

fn record(bytes: usize) {
    CALLS.fetch_add(1, Relaxed);
    BYTES.fetch_add(bytes as u64, Relaxed);
}

pub(super) fn snapshot() -> (u64, u64) {
    (CALLS.load(Relaxed), BYTES.load(Relaxed))
}

// SAFETY: каждый запрос и освобождение передаются тому же MiMalloc с исходным
// layout; счётчики не выделяют память и не меняют указатели или ошибки.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { mimalloc::MiMalloc.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { mimalloc::MiMalloc.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { mimalloc::MiMalloc.realloc(ptr, layout, size) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_wrapper_preserves_zeroing_reallocation_and_alignment() {
        let layout = Layout::from_size_align(32, 32).unwrap();
        // SAFETY: проверяем только успешно выделенную память в её границах;
        // последний layout соответствует realloc, освобождение однократное.
        unsafe {
            let ptr = Allocator.alloc_zeroed(layout);
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % 32, 0);
            assert!(std::slice::from_raw_parts(ptr, 32).iter().all(|b| *b == 0));
            ptr.write(17);
            let ptr = Allocator.realloc(ptr, layout, 64);
            assert!(!ptr.is_null());
            assert_eq!(ptr.read(), 17);
            Allocator.dealloc(ptr, Layout::from_size_align(64, 32).unwrap());
        }
    }
}
