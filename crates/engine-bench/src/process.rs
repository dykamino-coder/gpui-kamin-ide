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
        })
    }
    pub(super) fn json(self) -> Value {
        json!({"rust_allocations": self.allocations.json(),
            "rss_bytes": self.rss, "private_commit_bytes": self.commit,
            "process_lifetime_peak_rss_bytes": self.peak_rss,
            "process_lifetime_peak_commit_bytes": self.peak_commit})
    }
}
