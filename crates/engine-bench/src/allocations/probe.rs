//! Пробник стеков аллокаций: размер, указатель и стек запросов к Rust-аллокатору
//! в заданной фазе и диапазоне кадров замера, плюс освобождения. Выключен по
//! умолчанию; тогда аллокатор делает одно чтение флага, а сам пробник не
//! выделяет память даже при чтении окружения — счётчики аудита те же.
//!
//! `ENGINE_BENCH_ALLOC_STACKS=<фаза>[@<кадр>[-<кадр>]]`, где фаза — имя фазы
//! отчёта, `outside` (вне кадров замера; кадр 0 — старт процесса) или `*`;
//! кадры — порядковые номера кадров замера процесса с 1.
//! `ENGINE_BENCH_ALLOC_STACKS_UNTIL=<фаза>` — окно кончается, когда последний
//! кадр диапазона выходит из этой фазы (первого её вхождения): `*@0-2` с
//! `build_elements` — вся куча процесса на конец сборки второго кадра.
//! `ENGINE_BENCH_ALLOC_STACKS_SAMPLE=<байты>` — пуассоновская выборка: блок
//! пишется с вероятностью `1-exp(-размер/байты)`, освобождение — только у
//! выбранных блоков; без переменной пишется каждый блок (точный режим).
//! `ENGINE_BENCH_ALLOC_STACKS_OUT=<файл>` — куда писать строки
//! `<a|z|r> <кадр> <фаза> <байты> <указатель> <адреса возврата>` и для
//! освобождений `f <кадр> <фаза> <байты> <указатель>` (числа адресов — hex).
//! Освобождение пишется до возврата блока аллокатору, поэтому строка `f`
//! всегда стоит раньше строки повторного выделения того же адреса.
//! Сворачивает файл `scripts/memprof.py` по PDB той же сборки.

mod line;
mod sample;
mod sink;
mod window;

#[cfg(feature = "heap-report")]
pub(crate) use line::owned;
use line::{emit, head};
use window::{Filter, Until};

use kamin_html::work::Phase;
use std::{
    ffi::c_void,
    fmt::Write as _,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
};

const OUTSIDE: usize = usize::MAX;
const FRAMES: usize = 48;
const NAME: [u16; 26] = wide(b"ENGINE_BENCH_ALLOC_STACKS\0");
const OUT: &str = "ENGINE_BENCH_ALLOC_STACKS_OUT";
const UNTIL: &str = "ENGINE_BENCH_ALLOC_STACKS_UNTIL";
const SAMPLE: &str = "ENGINE_BENCH_ALLOC_STACKS_SAMPLE";

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
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn RtlCaptureStackBackTrace(
        skip: u32,
        capture: u32,
        trace: *mut *mut c_void,
        hash: *mut u32,
    ) -> u16;
}

struct Probe {
    filter: Filter,
    until: Option<usize>,
}
static PROBE: OnceLock<Probe> = OnceLock::new();
static ARMED: AtomicBool = AtomicBool::new(false);
static PHASE: AtomicUsize = AtomicUsize::new(OUTSIDE);
static FRAME: AtomicU64 = AtomicU64::new(0);
static WINDOW: std::sync::Mutex<Until> = std::sync::Mutex::new(Until::Open);

fn spec() -> Result<Option<String>, String> {
    let mut value = [0u16; 96];
    // SAFETY: имя завершено нулём, буфер и его длина принадлежат этому кадру стека.
    let length =
        unsafe { GetEnvironmentVariableW(NAME.as_ptr(), value.as_mut_ptr(), value.len() as u32) }
            as usize;
    if length == 0 {
        return Ok(None);
    }
    value
        .get(..length)
        .and_then(|units| String::from_utf16(units).ok())
        .map(Some)
        .ok_or_else(|| "ENGINE_BENCH_ALLOC_STACKS: expected <phase>[@<frame>[-<frame>]]".into())
}

fn phase_index(name: &str) -> Option<usize> {
    Phase::NAMES.iter().position(|known| *known == name)
}

/// Читает окружение до первой аллокации стенда. Без переменной не выделяет память.
pub(crate) fn init() -> Result<(), String> {
    let Some(spec) = spec()? else { return Ok(()) };
    let filter = Filter::parse(&spec, &Phase::NAMES).ok_or_else(|| {
        format!(
            "ENGINE_BENCH_ALLOC_STACKS={spec}: expected <phase>[@<frame>[-<frame>]], phase is * | outside | {}",
            Phase::NAMES.join(" | ")
        )
    })?;
    let until = match std::env::var(UNTIL) {
        Ok(name) => {
            Some(phase_index(&name).ok_or_else(|| format!("{UNTIL}={name}: unknown phase"))?)
        }
        Err(_) => None,
    };
    if until.is_some() && filter.last == u64::MAX {
        return Err(format!(
            "{UNTIL} needs a last frame in ENGINE_BENCH_ALLOC_STACKS"
        ));
    }
    let mean = match std::env::var(SAMPLE) {
        Ok(bytes) => bytes
            .parse::<u64>()
            .ok()
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| format!("{SAMPLE}={bytes}: expected a positive byte count"))?,
        Err(_) => 0,
    };
    let path = std::env::var_os(OUT).ok_or_else(|| format!("{OUT} is required with {spec}"))?;
    // SAFETY: нулевое имя означает модуль процесса; вызов ничего не захватывает.
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    let until_name = until.map_or("-", |index| Phase::NAMES[index]);
    sink::open(
        &path,
        &format!(
            "# engine_bench alloc stacks v3 base={base:x} filter={spec} until={until_name} sample={mean}\n"
        ),
    )?;
    sample::configure(mean);
    *WINDOW.lock().unwrap_or_else(|e| e.into_inner()) =
        until.map_or(Until::Open, |_| Until::Before);
    let _ = PROBE.set(Probe { filter, until });
    rearm();
    Ok(())
}

fn rearm() {
    let Some(probe) = PROBE.get() else { return };
    let (phase, frame) = (PHASE.load(Relaxed), FRAME.load(Relaxed));
    let mut window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(until) = probe.until {
        let next = window.step(until, probe.filter.last, phase, frame);
        if next == Until::Closed && *window != Until::Closed {
            ARMED.store(false, Relaxed);
            *window = next;
            drop(window);
            let name = Phase::NAMES[until];
            // Счётчик аудита в тот же момент: отчёт сверяет с ним, сколько кучи увидел.
            let live = super::Snapshot::read().live;
            let counter = if cfg!(feature = "allocation-audit") {
                format!(" live={live}")
            } else {
                String::new()
            };
            sink::trailer(&format!("# end frame={frame} after={name}{counter}\n"));
            return;
        }
        *window = next;
    }
    let on = *window != Until::Closed && probe.filter.matches(phase, frame);
    ARMED.store(on, Relaxed);
    drop(window);
    if !on {
        sink::flush();
    }
}

/// Начало кадра замера: следующий порядковый номер, фаза — как у учёта фаз.
pub(crate) fn frame() {
    FRAME.fetch_add(1, Relaxed);
    phase(Phase::PipelineOther as usize);
}
/// Граница фазы кадра замера.
pub(crate) fn phase(next: usize) {
    PHASE.store(next, Relaxed);
    rearm();
}
/// Кадр замера завершён; дальнейшие аллокации идут вне кадров.
pub(crate) fn outside() {
    phase(OUTSIDE);
}
/// Конец процесса: остаток буфера и число потерянных выборок — в файл.
pub(crate) fn finish() {
    if PROBE.get().is_some() {
        ARMED.store(false, Relaxed);
        sink::trailer(&format!("# dropped {}\n", sample::dropped()));
    }
}

/// Выделение из аллокатора: при выключенном пробнике — одно чтение флага.
#[inline]
pub(super) fn record(kind: u8, bytes: usize, pointer: *const u8) {
    if ARMED.load(Relaxed) && sample::take(bytes, pointer) {
        write(kind, bytes, pointer);
    }
}
/// Освобождение `pointer`: зовётся до того, как блок вернётся аллокатору.
#[inline]
pub(super) fn release(bytes: usize, pointer: *const u8) {
    if ARMED.load(Relaxed) && sample::forget(pointer) {
        write_release(bytes, pointer);
    }
}

#[cold]
#[inline(never)]
fn write(kind: u8, bytes: usize, pointer: *const u8) {
    let mut stack = [std::ptr::null_mut::<c_void>(); FRAMES];
    // SAFETY: буфер вмещает FRAMES адресов; хеш не запрошен.
    let depth = unsafe {
        RtlCaptureStackBackTrace(0, FRAMES as u32, stack.as_mut_ptr(), std::ptr::null_mut())
    } as usize;
    let (mut line, mut text) = head(kind, bytes, pointer);
    for address in &stack[..depth.min(FRAMES)] {
        text = text.and_then(|()| write!(line, " {:x}", *address as usize));
    }
    emit(line, text);
}

#[cold]
#[inline(never)]
fn write_release(bytes: usize, pointer: *const u8) {
    let (line, text) = head(b'f', bytes, pointer);
    emit(line, text);
}

#[cfg(test)]
mod tests;
