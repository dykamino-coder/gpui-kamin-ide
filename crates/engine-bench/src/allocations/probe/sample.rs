//! Пуассоновская выборка по байтам (как sampling heap profiler Chrome и
//! tcmalloc): у каждого потока счётчик байт до следующей выборки, интервалы —
//! экспоненциальные со средним `mean`; блок размера `s` выбирается с
//! вероятностью `1-exp(-s/mean)`, и отчёт делит на неё. Указатели выбранных
//! блоков хранятся в таблице без замков, чтобы писать освобождения только им.
//! Без выборки (`mean == 0`) пишется каждый блок и каждое освобождение.

use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
};

static MEAN: AtomicU64 = AtomicU64::new(0);
static SEEDS: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
static DROPPED: AtomicU64 = AtomicU64::new(0);

const SLOTS: usize = 1 << 21;
const PROBES: usize = 32;
const EMPTY: usize = 0;
const GONE: usize = 1;
static TABLE: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(EMPTY) }; SLOTS];

thread_local! {
    static UNTIL: Cell<i64> = const { Cell::new(0) };
    static STATE: Cell<u64> = const { Cell::new(0) };
}

pub(super) fn configure(mean: u64) {
    MEAN.store(mean, Relaxed);
}

pub(super) fn dropped() -> u64 {
    DROPPED.load(Relaxed)
}

/// splitmix64: равномерные 64 бита без аллокаций.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Экспоненциальный интервал со средним `mean` байт.
fn interval(state: &mut u64, mean: u64) -> i64 {
    let uniform = ((next(state) >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    (-uniform.ln() * mean as f64).min(i64::MAX as f64) as i64
}

/// Выбрать ли блок размера `bytes`; выбранный указатель запоминается.
#[inline]
pub(super) fn take(bytes: usize, pointer: *const u8) -> bool {
    let mean = MEAN.load(Relaxed);
    if mean == 0 {
        return true;
    }
    let chosen = UNTIL.with(|until| {
        let left = until.get() - bytes as i64;
        if left > 0 {
            until.set(left);
            return false;
        }
        STATE.with(|state| {
            let mut s = state.get();
            if s == 0 {
                s = SEEDS.fetch_add(0x6A09_E667_F3BC_C909, Relaxed) | 1;
            }
            // Перерасход текущего интервала переносится: выборка по байтам без смещения.
            let mut left = left;
            while left <= 0 {
                left += interval(&mut s, mean).max(1);
            }
            state.set(s);
            until.set(left);
        });
        true
    });
    chosen && insert(pointer as usize)
}

/// Писать ли освобождение: только выбранного блока.
#[inline]
pub(super) fn forget(pointer: *const u8) -> bool {
    MEAN.load(Relaxed) == 0 || remove(pointer as usize)
}

fn slot(pointer: usize, probe: usize) -> usize {
    let hash = (pointer as u64 >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 43;
    (hash as usize + probe) & (SLOTS - 1)
}

/// Живой блок вставляется один раз: другой поток не держит тот же адрес.
fn insert(pointer: usize) -> bool {
    for probe in 0..PROBES {
        let cell = &TABLE[slot(pointer, probe)];
        let seen = cell.load(Relaxed);
        if (seen == EMPTY || seen == GONE)
            && cell
                .compare_exchange(seen, pointer, Relaxed, Relaxed)
                .is_ok()
        {
            return true;
        }
    }
    DROPPED.fetch_add(1, Relaxed);
    false
}

fn remove(pointer: usize) -> bool {
    for probe in 0..PROBES {
        let cell = &TABLE[slot(pointer, probe)];
        match cell.load(Relaxed) {
            EMPTY => return false,
            seen if seen == pointer => {
                return cell
                    .compare_exchange(pointer, GONE, Relaxed, Relaxed)
                    .is_ok();
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_intervals_have_the_requested_mean() {
        let mut state = 7;
        let total: i64 = (0..200_000).map(|_| interval(&mut state, 4096)).sum();
        let mean = total as f64 / 200_000.;
        assert!((mean - 4096.).abs() < 4096. * 0.02, "{mean}");
    }

    #[test]
    fn a_remembered_pointer_is_forgotten_once() {
        let pointer = 0x7fff_1234_5670;
        assert!(insert(pointer));
        assert!(remove(pointer));
        assert!(!remove(pointer));
        assert!(!remove(pointer + 16));
    }
}
