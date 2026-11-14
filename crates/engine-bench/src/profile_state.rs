//! Положение стенда для внешнего профилировщика времени: номер кадра замера
//! и фаза в двух счётчиках с известным адресом. Сэмплер `scripts/timeprof.py`
//! останавливает поток окна и читает их из памяти процесса, поэтому сэмпл
//! всегда целиком принадлежит одной фазе. Граница фазы стоит две записи.
//!
//! `ENGINE_BENCH_PROFILE_STATE=<файл>` — стенд пишет туда строку
//! `v1 pid=<pid> state=<адрес hex> phases=<имена через запятую>`. Без
//! переменной чтение окружения не выделяет память: счётчики аудита те же.
//! Те же границы передаются пробнику аллокаций сборок с ним.

use kamin_html::work::Phase;
use std::{
    os::windows::ffi::OsStringExt,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

const OUTSIDE: u64 = u64::MAX;
const NAME: [u16; 27] = wide(b"ENGINE_BENCH_PROFILE_STATE\0");

const fn wide<const N: usize>(ascii: &[u8; N]) -> [u16; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = ascii[i] as u16;
        i += 1;
    }
    out
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetEnvironmentVariableW(name: *const u16, buffer: *mut u16, size: u32) -> u32;
}

/// Кадр замера (с 1 в порядке процесса) и индекс фазы в `Phase::NAMES`;
/// вне кадров замера фаза равна `u64::MAX`.
#[repr(C)]
struct State {
    frame: AtomicU64,
    phase: AtomicU64,
}
static STATE: State = State {
    frame: AtomicU64::new(0),
    phase: AtomicU64::new(OUTSIDE),
};

/// Сообщает профилировщику адрес счётчиков и имена фаз.
pub(crate) fn publish() -> Result<(), String> {
    let mut value = [0u16; 520];
    // SAFETY: имя завершено нулём, буфер и его длина принадлежат этому кадру стека.
    let length =
        unsafe { GetEnvironmentVariableW(NAME.as_ptr(), value.as_mut_ptr(), value.len() as u32) }
            as usize;
    let Some(path) = value.get(..length).filter(|path| !path.is_empty()) else {
        return Ok(());
    };
    let line = format!(
        "v1 pid={} state={:x} phases={}\n",
        std::process::id(),
        &raw const STATE as usize,
        Phase::NAMES.join(",")
    );
    std::fs::write(std::ffi::OsString::from_wide(path), line)
        .map_err(|error| format!("ENGINE_BENCH_PROFILE_STATE: {error}"))
}

/// Начало кадра замера.
pub(crate) fn frame() {
    STATE.frame.fetch_add(1, Relaxed);
    STATE.phase.store(Phase::PipelineOther as u64, Relaxed);
    #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
    super::allocations::probe::frame();
}

/// Граница фазы кадра замера.
pub(crate) fn phase(next: usize) {
    STATE.phase.store(next as u64, Relaxed);
    #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
    super::allocations::probe::phase(next);
}

/// Кадр замера завершён.
pub(crate) fn outside() {
    STATE.phase.store(OUTSIDE, Relaxed);
    #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
    super::allocations::probe::outside();
}
