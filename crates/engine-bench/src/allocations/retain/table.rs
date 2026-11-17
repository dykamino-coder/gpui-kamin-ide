//! Opt-in live-block table in system-allocator memory, excluded from the graph.
//! Tombstones preserve probe chains. Overflow invalidates the entire snapshot.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{
        AtomicBool, AtomicUsize,
        Ordering::{AcqRel, Acquire, Relaxed, Release},
    },
};

const EMPTY: usize = 0;
const TOMB: usize = 1;
const CHAIN: usize = 1 << 14;

#[repr(C)]
struct Slot {
    key: AtomicUsize,
    bytes: AtomicUsize,
}

/// Created only after the output variable is present; no table metadata is static.
pub(super) struct Table {
    pointer: *mut Slot,
    bits: u32,
    full: AtomicBool,
}

impl Table {
    pub(super) fn new(bits: u32) -> Result<Self, String> {
        let layout = Layout::array::<Slot>(1usize << bits).map_err(|error| error.to_string())?;
        // SAFETY: the nonempty allocation is zeroed, representing empty atomic slots.
        let pointer = unsafe { System.alloc_zeroed(layout) }.cast::<Slot>();
        if pointer.is_null() {
            return Err(format!(
                "retainers: cannot reserve {} slots",
                1usize << bits
            ));
        }
        Ok(Self {
            pointer,
            bits,
            full: AtomicBool::new(false),
        })
    }

    fn slots(&self) -> &[Slot] {
        // SAFETY: initialized before publication; retained for the process lifetime.
        // Slot contents are accessed only through atomics, including on other threads.
        unsafe { std::slice::from_raw_parts(self.pointer, self.capacity()) }
    }

    pub(super) fn insert(&self, pointer: usize, bytes: usize) {
        if self.overflowed() {
            return;
        }
        let slots = self.slots();
        let mask = slots.len() - 1;
        let mut index = home(pointer, self.bits);
        let mut steps = 0;
        while steps < CHAIN.min(slots.len()) {
            let slot = &slots[index];
            let key = slot.key.load(Acquire);
            if key == EMPTY || key == TOMB {
                if slot
                    .key
                    .compare_exchange(key, pointer, AcqRel, Acquire)
                    .is_ok()
                {
                    slot.bytes.store(bytes, Release);
                    return;
                }
                continue; // Another thread took this slot; inspect it again.
            }
            index = (index + 1) & mask;
            steps += 1;
        }
        self.full.store(true, Relaxed);
    }

    pub(super) fn remove(&self, pointer: usize) {
        if self.overflowed() {
            return;
        }
        let slots = self.slots();
        let mask = slots.len() - 1;
        let mut index = home(pointer, self.bits);
        for _ in 0..slots.len() {
            let slot = &slots[index];
            let key = slot.key.load(Acquire);
            if key == pointer {
                slot.bytes.store(0, Relaxed);
                slot.key.store(TOMB, Release);
                return;
            }
            if key == EMPTY {
                return;
            } // Allocated before tracking started.
            index = (index + 1) & mask;
        }
    }

    pub(super) fn overflowed(&self) -> bool {
        self.full.load(Relaxed)
    }
    pub(super) fn capacity(&self) -> usize {
        1usize << self.bits
    }

    /// Visits published live blocks; an insertion without its size is skipped.
    pub(super) fn each(&self, mut visit: impl FnMut(usize, usize)) {
        for slot in self.slots() {
            let key = slot.key.load(Acquire);
            if key > TOMB {
                let bytes = slot.bytes.load(Acquire);
                if bytes > 0 {
                    visit(key, bytes);
                }
            }
        }
    }
}

#[inline]
fn home(pointer: usize, bits: u32) -> usize {
    ((pointer as u64 >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - bits)) as usize
}
