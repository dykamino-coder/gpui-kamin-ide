//! Точные счётчики запросов глобальному Rust-аллокатору процесса стенда.
//! Native/D3D allocations в них не входят; WS/private commit измеряются отдельно.

#[cfg(feature = "allocation-audit")]
pub(super) use counted::Counted;

#[cfg(feature = "allocation-audit")]
mod counted {
    use super::Snapshot;
    use std::alloc::{GlobalAlloc, Layout};
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    pub(crate) struct Counted;
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static BYTES: AtomicU64 = AtomicU64::new(0);
    static FREED: AtomicU64 = AtomicU64::new(0);
    static LIVE: AtomicU64 = AtomicU64::new(0);
    static PEAK: AtomicU64 = AtomicU64::new(0);

    fn allocated(bytes: usize) {
        CALLS.fetch_add(1, Relaxed);
        BYTES.fetch_add(bytes as u64, Relaxed);
        let live = LIVE.fetch_add(bytes as u64, Relaxed) + bytes as u64;
        PEAK.fetch_max(live, Relaxed);
    }
    fn freed(bytes: usize) {
        FREED.fetch_add(bytes as u64, Relaxed);
        LIVE.fetch_sub(bytes as u64, Relaxed);
    }

    // Контракт GlobalAlloc передаётся MiMalloc без изменения layout/указателей.
    // Счётчики не выделяют память и не входят повторно в аллокатор.
    unsafe impl GlobalAlloc for Counted {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { mimalloc::MiMalloc.alloc(layout) };
            if !pointer.is_null() {
                allocated(layout.size());
            }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) };
            if !pointer.is_null() {
                allocated(layout.size());
            }
            pointer
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { mimalloc::MiMalloc.dealloc(pointer, layout) };
            freed(layout.size());
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let next = unsafe { mimalloc::MiMalloc.realloc(pointer, layout, size) };
            if !next.is_null() {
                // Размеры realloc учитываются полностью; failed realloc оставляет старый блок.
                freed(layout.size());
                allocated(size);
            }
            next
        }
    }

    pub(super) fn snapshot() -> Snapshot {
        Snapshot {
            calls: CALLS.load(Relaxed),
            bytes: BYTES.load(Relaxed),
            freed: FREED.load(Relaxed),
            live: LIVE.load(Relaxed),
            peak: PEAK.load(Relaxed),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Snapshot {
    pub(super) calls: u64,
    pub(super) bytes: u64,
    pub(super) freed: u64,
    pub(super) live: u64,
    pub(super) peak: u64,
}
impl Snapshot {
    pub(super) fn read() -> Self {
        #[cfg(feature = "allocation-audit")]
        {
            counted::snapshot()
        }
        #[cfg(not(feature = "allocation-audit"))]
        {
            Self::default()
        }
    }
    pub(super) fn json(self) -> serde_json::Value {
        if !cfg!(feature = "allocation-audit") {
            return serde_json::Value::Null;
        }
        serde_json::json!({"successful_alloc_or_realloc_calls": self.calls,
            "requested_bytes": self.bytes, "freed_requested_bytes": self.freed,
            "live_requested_bytes": self.live, "lifetime_peak_requested_bytes": self.peak})
    }
}

#[cfg(all(test, feature = "allocation-audit"))]
mod tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout};

    #[test]
    fn forwards_alignment_zeroing_and_reallocation() {
        let layout = Layout::from_size_align(31, 64).unwrap();
        unsafe {
            let pointer = Counted.alloc_zeroed(layout);
            assert!(!pointer.is_null());
            assert_eq!(pointer as usize % 64, 0);
            assert!(
                std::slice::from_raw_parts(pointer, 31)
                    .iter()
                    .all(|b| *b == 0)
            );
            *pointer = 42;
            let next = Counted.realloc(pointer, layout, 97);
            assert!(!next.is_null());
            assert_eq!(*next, 42);
            Counted.dealloc(next, Layout::from_size_align(97, 64).unwrap());
        }
    }
}

#[cfg(all(test, not(feature = "allocation-audit")))]
mod timing_tests {
    #[test]
    fn disabled_audit_reports_absence_instead_of_zero_allocations() {
        assert!(super::Snapshot::read().json().is_null());
    }
}
