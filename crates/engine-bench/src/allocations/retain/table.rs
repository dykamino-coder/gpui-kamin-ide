//! Таблица живых блоков `{адрес → размер}` для графа удержания: открытая
//! адресация без блокировок в памяти системного аллокатора (сама таблица не
//! проходит через шим и не попадает в граф). Удалённый ключ становится
//! надгробием; вставка занимает первое надгробие или пустой слот цепочки,
//! поэтому пустой слот никогда не появляется внутри цепочки живого ключа и
//! поиск останавливается на нём. Переполнение выключает учёт целиком: снимок
//! по неполной таблице не делается.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{
        AtomicBool, AtomicPtr, AtomicU32, AtomicUsize,
        Ordering::{AcqRel, Acquire, Relaxed, Release},
    },
};

const EMPTY: usize = 0;
const TOMB: usize = 1;
/// Длина цепочки вставки, после которой таблица считается переполненной.
const CHAIN: usize = 1 << 14;

#[repr(C)]
struct Slot {
    key: AtomicUsize,
    bytes: AtomicUsize,
}

static SLOTS: AtomicPtr<Slot> = AtomicPtr::new(std::ptr::null_mut());
static BITS: AtomicU32 = AtomicU32::new(0);
static TRACK: AtomicBool = AtomicBool::new(false);
static FULL: AtomicBool = AtomicBool::new(false);

/// Таблица на `1 << bits` слотов; учёт включается сразу.
pub(super) fn open(bits: u32) -> Result<(), String> {
    let layout = Layout::array::<Slot>(1usize << bits).map_err(|error| error.to_string())?;
    // SAFETY: размер ненулевой; нули — пустые слоты.
    let slots = unsafe { System.alloc_zeroed(layout) }.cast::<Slot>();
    if slots.is_null() {
        return Err(format!(
            "retainers: cannot reserve {} slots",
            1usize << bits
        ));
    }
    BITS.store(bits, Relaxed);
    SLOTS.store(slots, Release);
    TRACK.store(true, Release);
    Ok(())
}

#[inline]
fn slots() -> Option<(&'static [Slot], u32)> {
    let pointer = SLOTS.load(Acquire);
    if pointer.is_null() {
        return None;
    }
    let bits = BITS.load(Relaxed);
    // SAFETY: таблица выделена в `open` и не освобождается до конца процесса.
    Some((
        unsafe { std::slice::from_raw_parts(pointer, 1usize << bits) },
        bits,
    ))
}

#[inline]
fn home(pointer: usize, bits: u32) -> usize {
    ((pointer as u64 >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - bits)) as usize
}

/// Выключенный учёт — одно чтение флага.
#[inline]
pub(super) fn tracking() -> bool {
    TRACK.load(Relaxed)
}

pub(super) fn insert(pointer: usize, bytes: usize) {
    let Some((slots, bits)) = slots() else { return };
    let mask = slots.len() - 1;
    let mut index = home(pointer, bits);
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
            continue; // слот занял другой поток: смотрим его снова
        }
        index = (index + 1) & mask;
        steps += 1;
    }
    TRACK.store(false, Relaxed);
    FULL.store(true, Relaxed);
}

pub(super) fn remove(pointer: usize) {
    let Some((slots, bits)) = slots() else { return };
    let mask = slots.len() - 1;
    let mut index = home(pointer, bits);
    for _ in 0..slots.len() {
        let slot = &slots[index];
        let key = slot.key.load(Acquire);
        if key == pointer {
            slot.bytes.store(0, Relaxed);
            slot.key.store(TOMB, Release);
            return;
        }
        if key == EMPTY {
            return; // блок выделен до включения учёта
        }
        index = (index + 1) & mask;
    }
}

/// Переполнялась ли таблица (тогда учёт выключен и снимок не делается).
pub(super) fn overflowed() -> bool {
    FULL.load(Relaxed)
}

/// Число слотов таблицы.
pub(super) fn capacity() -> usize {
    slots().map_or(0, |(slots, _)| slots.len())
}

/// Каждый живой блок `(адрес, байты)`; блок, который вставляется прямо сейчас
/// и ещё без размера, пропускается.
pub(super) fn each(mut visit: impl FnMut(usize, usize)) {
    let Some((slots, _)) = slots() else { return };
    for slot in slots {
        let key = slot.key.load(Acquire);
        if key > TOMB {
            let bytes = slot.bytes.load(Acquire);
            if bytes > 0 {
                visit(key, bytes);
            }
        }
    }
}
