//! KaminIDE patch: heap blocks a window holds, for the engine bench's memory report
//! (`ENGINE_BENCH_MEMORY_REPORT`, docs/memprof.md). Walked only on request, nothing is
//! counted in the hot structures. Sizes are requested bytes; the address is null where
//! the container does not expose its block. Values inside the element arena and boxed
//! listeners are not walked: their own heap stays in the unclassified remainder.

use std::mem::size_of;

/// Receives `(owner path, block address, requested bytes)`.
pub type HeapSink<'a> = dyn FnMut(&'static str, *const u8, usize) + 'a;

/// The buffer of a vector.
pub(crate) fn vec<T>(sink: &mut HeapSink, path: &'static str, items: &Vec<T>) {
    if items.capacity() > 0 && size_of::<T>() > 0 {
        sink(path, items.as_ptr().cast(), items.capacity() * size_of::<T>());
    }
}

/// The table of a hashbrown map or set of `capacity` with entries of `T`: slots, control
/// bytes and one trailing group (SSE2 group width 16).
pub(crate) fn table<T>(sink: &mut HeapSink, path: &'static str, capacity: usize) {
    if capacity == 0 {
        return;
    }
    let buckets = if capacity < 8 { capacity + 1 } else { capacity / 7 * 8 };
    let bytes = (buckets * size_of::<T>()).next_multiple_of(16) + buckets + 16;
    sink(path, std::ptr::null(), bytes);
}

/// The block of a `Box<dyn …>`: the value's own size; what it owns is not walked.
pub(crate) fn boxed<T: ?Sized>(sink: &mut HeapSink, path: &'static str, value: &T) {
    let bytes = std::mem::size_of_val(value);
    if bytes > 0 {
        sink(path, (value as *const T).cast(), bytes);
    }
}

/// The block of an `Rc`/`Arc` whose value is at `value`: two counters before it.
pub(crate) fn counted<T>(sink: &mut HeapSink, path: &'static str, value: *const T) {
    let header = (2 * size_of::<usize>()).next_multiple_of(align_of::<T>());
    let bytes = (header + size_of::<T>()).next_multiple_of(align_of::<T>().max(align_of::<usize>()));
    sink(path, value.cast::<u8>().wrapping_sub(header), bytes);
}
