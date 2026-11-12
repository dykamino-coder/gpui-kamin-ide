//! Лёгкий профиль кучи для timing-подобной сборки (`heap-profile`): без
//! атомарных счётчиков аудита, только пробник стеков, обычно в режиме выборки
//! (`ENGINE_BENCH_ALLOC_STACKS_SAMPLE`). Выключенный пробник — одно чтение флага.

use std::alloc::{GlobalAlloc, Layout};

pub(crate) struct Sampled;

#[inline]
fn record(kind: u8, bytes: usize, pointer: *const u8) {
    #[cfg(windows)]
    super::probe::record(kind, bytes, pointer);
    #[cfg(not(windows))]
    let _ = (kind, bytes, pointer);
}
#[inline]
fn release(bytes: usize, pointer: *const u8) {
    #[cfg(windows)]
    super::probe::release(bytes, pointer);
    #[cfg(not(windows))]
    let _ = (bytes, pointer);
}

// Контракт GlobalAlloc передаётся MiMalloc без изменения layout/указателей.
unsafe impl GlobalAlloc for Sampled {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { mimalloc::MiMalloc.alloc(layout) };
        if !pointer.is_null() {
            record(b'a', layout.size(), pointer);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record(b'z', layout.size(), pointer);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // До возврата блока: другой поток не может получить адрес раньше строки `f`.
        release(layout.size(), pointer);
        unsafe { mimalloc::MiMalloc.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        release(layout.size(), pointer);
        let next = unsafe { mimalloc::MiMalloc.realloc(pointer, layout, size) };
        if next.is_null() {
            // Старый блок остался у вызывающего.
            record(b'a', layout.size(), pointer);
        } else {
            record(b'r', size, next);
        }
        next
    }
}
