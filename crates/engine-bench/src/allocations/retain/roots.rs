//! Области корней графа удержания — единственное платформенное место скана:
//! стек потока, который делает снимок (от его текущей вершины до основания),
//! записываемые секции образа процесса (статики) и TLS этого потока.
//! Windows: границы стека потока, секции и каталог TLS из PE-заголовка
//! образа. Linux: `/proc/self/maps` (стек основного потока и записываемые
//! отображения исполняемого файла с его `.bss`). Стеки других потоков и память
//! ОС не сканируются: то, что держат только они, входит в граф через «без корня».

use kamin_heap::retain::Region;

const WORD: usize = size_of::<usize>();

fn region(name: impl Into<String>, start: usize, end: usize) -> Option<Region> {
    let start = start.next_multiple_of(WORD);
    (end > start).then(|| Region {
        name: name.into(),
        start,
        bytes: (end - start) / WORD * WORD,
    })
}

#[cfg(windows)]
pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
    windows::regions(stack_pointer)
}

#[cfg(target_os = "linux")]
pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
    linux::regions(stack_pointer)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(super) fn regions(_stack_pointer: usize) -> Vec<Region> {
    Vec::new()
}

#[cfg(windows)]
mod windows {
    use super::{Region, region};
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    const WRITE: u32 = 0x8000_0000;
    const TLS_DIRECTORY: usize = 9;

    fn u16_at(address: usize) -> u16 {
        // SAFETY: адрес внутри заголовков загруженного образа процесса.
        unsafe { std::ptr::read_unaligned(address as *const u16) }
    }
    fn u32_at(address: usize) -> u32 {
        // SAFETY: как у `u16_at`.
        unsafe { std::ptr::read_unaligned(address as *const u32) }
    }
    fn u64_at(address: usize) -> u64 {
        // SAFETY: как у `u16_at`.
        unsafe { std::ptr::read_unaligned(address as *const u64) }
    }

    pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
        let mut out = Vec::new();
        let (mut low, mut high) = (0usize, 0usize);
        // SAFETY: функция только пишет два слова по переданным адресам.
        unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
        if (low..high).contains(&stack_pointer) {
            out.extend(region("stack of the snapshot thread", stack_pointer, high));
        }
        // SAFETY: нулевое имя — модуль процесса; вызов ничего не захватывает.
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        if base == 0 || u16_at(base) != 0x5a4d {
            return out;
        }
        let nt = base + u32_at(base + 0x3c) as usize;
        if u32_at(nt) != 0x4550 || u16_at(nt + 24) != 0x20b {
            return out; // не PE32+
        }
        let sections = u16_at(nt + 6) as usize;
        let optional = nt + 24;
        let table = optional + u16_at(nt + 20) as usize;
        for index in 0..sections {
            let header = table + index * 40;
            if u32_at(header + 36) & WRITE == 0 {
                continue;
            }
            // SAFETY: имя секции — 8 байт заголовка, дополненные нулями.
            let raw = unsafe { std::slice::from_raw_parts(header as *const u8, 8) };
            let name = String::from_utf8_lossy(raw).trim_end_matches('\0').to_owned();
            let start = base + u32_at(header + 12) as usize;
            let size = u32_at(header + 8) as usize;
            out.extend(region(format!("static {name}"), start, start + size));
        }
        out.extend(thread_local(base, optional));
        out
    }

    /// Блок TLS образа у текущего потока: `TEB.ThreadLocalStoragePointer[индекс]`.
    #[cfg(target_arch = "x86_64")]
    fn thread_local(base: usize, optional: usize) -> Option<Region> {
        let directory = optional + 112 + TLS_DIRECTORY * 8;
        let (rva, size) = (u32_at(directory) as usize, u32_at(directory + 4));
        if rva == 0 || size == 0 {
            return None;
        }
        let tls = base + rva;
        let (start, end) = (u64_at(tls) as usize, u64_at(tls + 8) as usize);
        let index = u32_at(u64_at(tls + 16) as usize) as usize;
        let zero = u32_at(tls + 32) as usize;
        let slots: usize;
        // SAFETY: gs указывает на TEB текущего потока; 0x58 — ThreadLocalStoragePointer.
        unsafe {
            std::arch::asm!("mov {}, gs:[0x58]", out(reg) slots, options(nostack, readonly, preserves_flags));
        }
        if slots == 0 {
            return None;
        }
        // SAFETY: массив TLS потока содержит слот каждого модуля со статическим TLS.
        let block = unsafe { *(slots as *const usize).add(index) };
        if block == 0 {
            return None;
        }
        let bytes = end.saturating_sub(start) + zero;
        region("thread-local of the snapshot thread", block, block + bytes)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn thread_local(_base: usize, _optional: usize) -> Option<Region> {
        None
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Region, region};

    /// `start-end perms offset dev inode path` одной строки `/proc/self/maps`.
    fn parse(line: &str) -> Option<(usize, usize, &str, &str)> {
        let mut fields = line.split_whitespace();
        let (start, end) = fields.next()?.split_once('-')?;
        let perms = fields.next()?;
        let path = fields.nth(3).unwrap_or("");
        let start = usize::from_str_radix(start, 16).ok()?;
        let end = usize::from_str_radix(end, 16).ok()?;
        Some((start, end, perms, path))
    }

    pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
        let mut out = Vec::new();
        let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else {
            return out;
        };
        let exe = std::env::current_exe().ok();
        let exe = exe.as_deref().and_then(|path| path.to_str()).unwrap_or("");
        let mut after_exe = false;
        for (start, end, perms, path) in maps.lines().filter_map(parse) {
            let own = !exe.is_empty() && path == exe;
            if path == "[stack]" && (start..end).contains(&stack_pointer) {
                out.extend(region("stack of the snapshot thread", stack_pointer, end));
            } else if perms.starts_with("rw") && own {
                out.extend(region(format!("static {path}"), start, end));
            } else if perms.starts_with("rw") && path.is_empty() && after_exe {
                out.extend(region("static .bss", start, end));
            }
            after_exe = own;
        }
        out
    }
}
