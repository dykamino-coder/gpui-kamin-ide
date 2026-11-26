//! Platform-specific retention roots: the snapshot thread stack from its current
//! position to its base, writable image sections, and thread-local storage.
//! Windows uses stack bounds and PE section/TLS headers. Linux uses
//! `/proc/self/maps` for the main stack and writable executable mappings,
//! including `.bss`. Other threads and OS memory are not scanned; blocks held
//! only there enter the graph through the unrooted node.

use kamin_heap::retain::Region;

const WORD: usize = size_of::<usize>();

#[cfg(test)]
mod tests;

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
    excluding(
        windows::regions(stack_pointer),
        super::super::probe::pointer_table_range(),
    )
}

/// Metadata records raw addresses without keeping the allocations alive.
#[cfg(any(windows, target_os = "linux", test))]
fn excluding(regions: Vec<Region>, metadata: std::ops::Range<usize>) -> Vec<Region> {
    let mut out = Vec::new();
    for source in regions {
        let end = source.start + source.bytes;
        if source.start >= metadata.end || end <= metadata.start {
            out.push(source);
            continue;
        }
        out.extend(region(
            source.name.clone(),
            source.start,
            metadata.start.min(end),
        ));
        out.extend(region(source.name, metadata.end.max(source.start), end));
    }
    out
}

#[cfg(target_os = "linux")]
pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
    excluding(
        linux::regions(stack_pointer),
        super::super::probe::pointer_table_range(),
    )
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
        // SAFETY: the address lies within the loaded process image headers.
        unsafe { std::ptr::read_unaligned(address as *const u16) }
    }
    fn u32_at(address: usize) -> u32 {
        // SAFETY: the same loaded-image bounds as `u16_at` apply.
        unsafe { std::ptr::read_unaligned(address as *const u32) }
    }
    fn u64_at(address: usize) -> u64 {
        // SAFETY: the same loaded-image bounds as `u16_at` apply.
        unsafe { std::ptr::read_unaligned(address as *const u64) }
    }

    pub(super) fn regions(stack_pointer: usize) -> Vec<Region> {
        let mut out = Vec::new();
        let (mut low, mut high) = (0usize, 0usize);
        // SAFETY: the function writes only two words to the supplied output addresses.
        unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
        if (low..high).contains(&stack_pointer) {
            out.extend(region("stack of the snapshot thread", stack_pointer, high));
        }
        // SAFETY: a null name selects the process module; the call retains no pointers.
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        if base == 0 || u16_at(base) != 0x5a4d {
            return out;
        }
        let nt = base + u32_at(base + 0x3c) as usize;
        if u32_at(nt) != 0x4550 || u16_at(nt + 24) != 0x20b {
            return out; // Not PE32+.
        }
        let sections = u16_at(nt + 6) as usize;
        let optional = nt + 24;
        let table = optional + u16_at(nt + 20) as usize;
        for index in 0..sections {
            let header = table + index * 40;
            if u32_at(header + 36) & WRITE == 0 {
                continue;
            }
            // SAFETY: the section name occupies eight zero-padded header bytes.
            let raw = unsafe { std::slice::from_raw_parts(header as *const u8, 8) };
            let name = String::from_utf8_lossy(raw)
                .trim_end_matches('\0')
                .to_owned();
            let start = base + u32_at(header + 12) as usize;
            let size = u32_at(header + 8) as usize;
            out.extend(region(format!("static {name}"), start, start + size));
        }
        out.extend(thread_local(base, optional));
        out
    }

    /// The current thread image TLS block: `TEB.ThreadLocalStoragePointer[index]`.
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
        // SAFETY: gs points to this thread TEB; offset 0x58 is ThreadLocalStoragePointer.
        unsafe {
            std::arch::asm!("mov {}, gs:[0x58]", out(reg) slots, options(nostack, readonly, preserves_flags));
        }
        if slots == 0 {
            return None;
        }
        // SAFETY: the thread TLS array contains a slot for each static-TLS module.
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

    /// Parse `start-end perms offset dev inode path` from one `/proc/self/maps` line.
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
