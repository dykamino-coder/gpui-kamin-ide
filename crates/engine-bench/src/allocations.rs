//! РўРѕС‡РЅС‹Рµ СЃС‡С‘С‚С‡РёРєРё Р·Р°РїСЂРѕСЃРѕРІ РіР»РѕР±Р°Р»СЊРЅРѕРјСѓ Rust-Р°Р»Р»РѕРєР°С‚РѕСЂСѓ РїСЂРѕС†РµСЃСЃР° СЃС‚РµРЅРґР°.
//! Native/D3D allocations РІ РЅРёС… РЅРµ РІС…РѕРґСЏС‚; WS/private commit РёР·РјРµСЂСЏСЋС‚СЃСЏ РѕС‚РґРµР»СЊРЅРѕ.

#[cfg(feature = "allocation-audit")]
pub(super) use counted::Counted;
#[cfg(all(test, feature = "allocation-audit"))]
pub(super) use counted::thread_live;

#[cfg(all(windows, any(feature = "allocation-audit", feature = "heap-profile")))]
pub(super) mod probe;

#[cfg(all(feature = "heap-profile", not(feature = "allocation-audit")))]
mod sampled;
#[cfg(all(feature = "heap-profile", not(feature = "allocation-audit")))]
pub(super) use sampled::Sampled;

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

    // Tests only: live bytes by thread, so a test can check its own thread while others run.
    #[cfg(test)]
    thread_local! {
        static THREAD_LIVE: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    }
    #[cfg(test)]
    fn thread_delta(bytes: i64) {
        let _ = THREAD_LIVE.try_with(|live| live.set(live.get() + bytes));
    }
    /// Bytes this thread allocated and has not freed (tests only).
    #[cfg(test)]
    pub(crate) fn thread_live() -> i64 {
        THREAD_LIVE.with(std::cell::Cell::get)
    }

    fn allocated(bytes: usize) {
        #[cfg(test)]
        thread_delta(bytes as i64);
        CALLS.fetch_add(1, Relaxed);
        BYTES.fetch_add(bytes as u64, Relaxed);
        let live = LIVE.fetch_add(bytes as u64, Relaxed) + bytes as u64;
        PEAK.fetch_max(live, Relaxed);
    }
    // Пробник стеков выключен по умолчанию: тогда это одно чтение флага.
    #[inline]
    fn stack(kind: u8, bytes: usize, pointer: *const u8) {
        #[cfg(windows)]
        super::probe::record(kind, bytes, pointer);
        #[cfg(not(windows))]
        let _ = (kind, bytes, pointer);
    }
    // Освобождение пишется до возврата блока mimalloc: иначе другой поток мог бы
    // получить тот же адрес и записать его выделение раньше этого освобождения.
    #[inline]
    fn release(bytes: usize, pointer: *const u8) {
        #[cfg(windows)]
        super::probe::release(bytes, pointer);
        #[cfg(not(windows))]
        let _ = (bytes, pointer);
    }
    fn freed(bytes: usize) {
        #[cfg(test)]
        thread_delta(-(bytes as i64));
        FREED.fetch_add(bytes as u64, Relaxed);
        LIVE.fetch_sub(bytes as u64, Relaxed);
    }

    // РљРѕРЅС‚СЂР°РєС‚ GlobalAlloc РїРµСЂРµРґР°С‘С‚СЃСЏ MiMalloc Р±РµР· РёР·РјРµРЅРµРЅРёСЏ layout/СѓРєР°Р·Р°С‚РµР»РµР№.
    // РЎС‡С‘С‚С‡РёРєРё РЅРµ РІС‹РґРµР»СЏСЋС‚ РїР°РјСЏС‚СЊ Рё РЅРµ РІС…РѕРґСЏС‚ РїРѕРІС‚РѕСЂРЅРѕ РІ Р°Р»Р»РѕРєР°С‚РѕСЂ.
    unsafe impl GlobalAlloc for Counted {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { mimalloc::MiMalloc.alloc(layout) };
            if !pointer.is_null() {
                allocated(layout.size());
                stack(b'a', layout.size(), pointer);
            }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) };
            if !pointer.is_null() {
                allocated(layout.size());
                stack(b'z', layout.size(), pointer);
            }
            pointer
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            release(layout.size(), pointer);
            unsafe { mimalloc::MiMalloc.dealloc(pointer, layout) };
            freed(layout.size());
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            release(layout.size(), pointer);
            let next = unsafe { mimalloc::MiMalloc.realloc(pointer, layout, size) };
            if !next.is_null() {
                // Р Р°Р·РјРµСЂС‹ realloc СѓС‡РёС‚С‹РІР°СЋС‚СЃСЏ РїРѕР»РЅРѕСЃС‚СЊСЋ; failed realloc РѕСЃС‚Р°РІР»СЏРµС‚ СЃС‚Р°СЂС‹Р№ Р±Р»РѕРє.
                freed(layout.size());
                allocated(size);
                stack(b'r', size, next);
            } else {
                // Старый блок остался у вызывающего: пробник снова считает его живым.
                stack(b'a', layout.size(), pointer);
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

// SVG-интервалы читают тот же opt-in аудит, что и снимки памяти фаз.
// В timing-сборке нет ни callback, ни атомарных операций в аллокаторе.
#[cfg(feature = "work-counters")]
pub(super) fn svg_observer() -> Option<fn() -> (u64, u64)> {
    #[cfg(feature = "allocation-audit")]
    {
        Some(|| {
            let snapshot = Snapshot::read();
            (snapshot.calls, snapshot.bytes)
        })
    }
    #[cfg(not(feature = "allocation-audit"))]
    {
        None
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
    fn svg_intervals_use_the_opt_in_allocation_audit() {
        let observer = svg_observer().expect("allocation-audit provides SVG samples");
        let before = observer();
        let allocation = std::hint::black_box(vec![0u8; 1024]);
        let after = observer();
        assert!(after.0 > before.0);
        assert!(after.1 >= before.1 + 1024);
        drop(allocation);
    }

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
        #[cfg(feature = "work-counters")]
        assert!(super::svg_observer().is_none());
    }
}
