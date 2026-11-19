//! Test builds: the paint path's counts of the calling thread alone, so that
//! tests running in parallel do not see each other's paths (`read`).

use std::cell::Cell;

const N: usize = super::NAMES.len();

thread_local! {
    static COUNTS: Cell<[u64; N]> = const { Cell::new([0; N]) };
}

/// Add `n` to counter `i` (in the order of `NAMES`) of this thread.
pub(super) fn add(i: usize, n: u64) {
    COUNTS.with(|c| {
        let mut counts = c.get();
        counts[i] += n;
        c.set(counts);
    });
}

/// This thread's counts in the order of `NAMES`.
pub(super) fn read() -> [u64; N] {
    COUNTS.with(Cell::get)
}
