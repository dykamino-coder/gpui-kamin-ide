//! Счётчики Windows всего процесса: CPU всех потоков и память, без обхода DOM.

use serde_json::{Value, json};
#[cfg(feature = "work-counters")]
pub(super) mod gpu;
use windows::Win32::{
    Foundation::FILETIME,
    System::{
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        Threading::{GetCurrentProcess, GetProcessTimes},
    },
};

#[derive(Clone, Copy, Default)]
pub(super) struct Snapshot {
    pub(super) allocations: super::allocations::Snapshot,
    pub(super) cpu_100ns: u64,
    pub(super) rss: u64,
    pub(super) commit: u64,
    pub(super) peak_rss: u64,
    pub(super) peak_commit: u64,
    pub(super) allocator: Option<kamin_alloc::Memory>,
}

impl Snapshot {
    pub(super) fn read() -> windows::core::Result<Self> {
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        // Псевдодескриптор своего процесса; буферы живут до возврата API.
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )?;
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
                memory.cb,
            )?;
        }
        let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        Ok(Self {
            allocations: super::allocations::Snapshot::read(),
            cpu_100ns: ticks(kernel) + ticks(user),
            rss: memory.WorkingSetSize as u64,
            commit: memory.PrivateUsage as u64,
            peak_rss: memory.PeakWorkingSetSize as u64,
            peak_commit: memory.PeakPagefileUsage as u64,
            allocator: allocator_requested().then(kamin_alloc::memory).flatten(),
        })
    }
    pub(super) fn json(self) -> Value {
        let mut out = json!({"rust_allocations": self.allocations.json(),
            "rss_bytes": self.rss, "private_commit_bytes": self.commit,
            "process_lifetime_peak_rss_bytes": self.peak_rss,
            "process_lifetime_peak_commit_bytes": self.peak_commit});
        if let Some(memory) = self.allocator {
            out["allocator"] = json!({"reserved_bytes": memory.reserved,
                "committed_bytes": memory.committed,
                "peak_committed_bytes": memory.peak_committed,
                "page_committed_bytes": memory.page_committed,
                "purged_bytes": memory.purged});
        }
        out
    }
}

/// Память аллокатора входит в снимки только по `ENGINE_BENCH_ALLOCATOR_MEMORY`:
/// её значения зависят от момента очистки арен, а остальной отчёт сравнивают
/// между сборками поле в поле. Окружение читается без аллокаций.
fn allocator_requested() -> bool {
    static ASKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ASKED.get_or_init(|| {
        let mut value = [0u16; 8];
        // SAFETY: имя завершено нулём, буфер и его длина принадлежат этому кадру стека.
        unsafe { GetEnvironmentVariableW(NAME.as_ptr(), value.as_mut_ptr(), value.len() as u32) != 0 }
    })
}

const NAME: [u16; 30] = {
    let ascii = b"ENGINE_BENCH_ALLOCATOR_MEMORY\0";
    let mut out = [0; 30];
    let mut i = 0;
    while i < 30 {
        out[i] = ascii[i] as u16;
        i += 1;
    }
    out
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetEnvironmentVariableW(name: *const u16, buffer: *mut u16, size: u32) -> u32;
}
