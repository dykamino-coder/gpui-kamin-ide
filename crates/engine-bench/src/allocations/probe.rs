//! Пробник стеков аллокаций: размер и стек каждого запроса к Rust-аллокатору
//! в заданной фазе и диапазоне кадров замера. Выключен по умолчанию; тогда
//! аллокатор делает одно чтение флага, а сам пробник не выделяет память даже
//! при чтении окружения — счётчики аудита остаются теми же.
//!
//! `ENGINE_BENCH_ALLOC_STACKS=<фаза>[@<кадр>[-<кадр>]]`, где фаза — имя фазы
//! отчёта, `outside` (вне кадров замера; кадр 0 — старт процесса) или `*`;
//! кадры — порядковые номера кадров замера процесса с 1.
//! `ENGINE_BENCH_ALLOC_STACKS_OUT=<файл>` — куда писать строки
//! `<a|z|r> <кадр> <фаза> <байты> <адреса возврата, hex>`. Адреса переводит в
//! имена `scripts/alloc_stacks.py` по PDB той же сборки.

use kamin_html::work::Phase;
use std::{
    ffi::c_void,
    fmt::Write as _,
    fs::File,
    io::Write as _,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
};

const OUTSIDE: usize = usize::MAX;
const FRAMES: usize = 48;
const NAME: [u16; 26] = wide(b"ENGINE_BENCH_ALLOC_STACKS\0");
const OUT: &str = "ENGINE_BENCH_ALLOC_STACKS_OUT";

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

/// Какие аллокации записывать: фаза (`None` — любая) и кадры замера.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Filter {
    phase: Option<usize>,
    first: u64,
    last: u64,
}
impl Filter {
    fn parse(spec: &str, names: &[&str]) -> Option<Self> {
        let (phase, frames) = match spec.split_once('@') {
            Some((phase, frames)) => (phase, Some(frames)),
            None => (spec, None),
        };
        let phase = match phase {
            "*" => None,
            "outside" => Some(OUTSIDE),
            name => Some(names.iter().position(|known| *known == name)?),
        };
        let (first, last) = match frames.map(|frames| frames.split_once('-').ok_or(frames)) {
            None => (0, u64::MAX),
            Some(Ok((first, last))) => (first.parse().ok()?, last.parse().ok()?),
            Some(Err(one)) => {
                let one = one.parse().ok()?;
                (one, one)
            }
        };
        (first <= last).then_some(Self { phase, first, last })
    }
    fn matches(self, phase: usize, frame: u64) -> bool {
        self.phase.is_none_or(|wanted| wanted == phase) && (self.first..=self.last).contains(&frame)
    }
}

struct Probe {
    filter: Filter,
    file: File,
}
static PROBE: OnceLock<Probe> = OnceLock::new();
static ARMED: AtomicBool = AtomicBool::new(false);
static PHASE: AtomicUsize = AtomicUsize::new(OUTSIDE);
static FRAME: AtomicU64 = AtomicU64::new(0);

/// Читает окружение до первой аллокации стенда. Без переменной не выделяет память.
pub(crate) fn init() -> Result<(), String> {
    let mut value = [0u16; 96];
    // SAFETY: имя завершено нулём, буфер и его длина принадлежат этому кадру стека.
    let length =
        unsafe { GetEnvironmentVariableW(NAME.as_ptr(), value.as_mut_ptr(), value.len() as u32) }
            as usize;
    if length == 0 {
        return Ok(());
    }
    let mut ascii = [0u8; 96];
    let spec = value
        .get(..length)
        .filter(|units| units.iter().all(|unit| *unit < 128))
        .map(|units| {
            for (byte, unit) in ascii.iter_mut().zip(units) {
                *byte = *unit as u8;
            }
            &ascii[..length]
        })
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or("ENGINE_BENCH_ALLOC_STACKS: expected <phase>[@<frame>[-<frame>]]")?;
    let filter = Filter::parse(spec, &Phase::NAMES).ok_or_else(|| {
        format!(
            "ENGINE_BENCH_ALLOC_STACKS={spec}: expected <phase>[@<frame>[-<frame>]], phase is * | outside | {}",
            Phase::NAMES.join(" | ")
        )
    })?;
    let path = std::env::var_os(OUT).ok_or_else(|| format!("{OUT} is required with {spec}"))?;
    let open = |error: std::io::Error| format!("{}: {error}", path.to_string_lossy());
    let mut file = File::create(&path).map_err(open)?;
    // SAFETY: нулевое имя означает модуль процесса; вызов ничего не захватывает.
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    writeln!(
        file,
        "# engine_bench alloc stacks v1 base={base:x} filter={spec}"
    )
    .map_err(open)?;
    drop(file);
    // Дозапись: строки потоков не перемешиваются и не затирают друг друга.
    let file = File::options().append(true).open(&path).map_err(open)?;
    let _ = PROBE.set(Probe { filter, file });
    rearm();
    Ok(())
}

fn rearm() {
    if let Some(probe) = PROBE.get() {
        let on = probe
            .filter
            .matches(PHASE.load(Relaxed), FRAME.load(Relaxed));
        ARMED.store(on, Relaxed);
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

/// Вызов из аллокатора: при выключенном пробнике — одно чтение флага.
#[inline]
pub(super) fn record(kind: u8, bytes: usize) {
    if ARMED.load(Relaxed) {
        write(kind, bytes);
    }
}

/// Строка в буфере на стеке: запись из аллокатора не может выделять память.
struct Line {
    bytes: [u8; 1024],
    length: usize,
}
impl std::fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.length + text.len();
        self.bytes
            .get_mut(self.length..end)
            .ok_or(std::fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

#[cold]
#[inline(never)]
fn write(kind: u8, bytes: usize) {
    let Some(probe) = PROBE.get() else { return };
    let mut stack = [std::ptr::null_mut::<c_void>(); FRAMES];
    // SAFETY: буфер вмещает FRAMES адресов; хеш не запрошен.
    let depth = unsafe {
        RtlCaptureStackBackTrace(0, FRAMES as u32, stack.as_mut_ptr(), std::ptr::null_mut())
    } as usize;
    let phase = PHASE.load(Relaxed);
    let name = Phase::NAMES.get(phase).copied().unwrap_or("outside");
    let mut line = Line {
        bytes: [0; 1024],
        length: 0,
    };
    let frame = FRAME.load(Relaxed);
    let mut text = write!(line, "{} {frame} {name} {bytes}", kind as char);
    for address in &stack[..depth.min(FRAMES)] {
        text = text.and_then(|()| write!(line, " {:x}", *address as usize));
    }
    if text.and_then(|()| line.write_str("\n")).is_ok() {
        let _ = (&probe.file).write_all(&line.bytes[..line.length]);
    }
}

#[cfg(test)]
mod tests;
