//! Счётчики Windows всего процесса: CPU всех потоков и память, без обхода DOM.

use serde_json::{Value, json};
#[cfg(feature = "work-counters")]
pub(super) mod gpu;

/// CPU cycles of every thread of this process so far (QueryProcessCycleTime).
pub(super) fn cycles() -> u64 {
    kamin_profiler::process::cycles().value.unwrap_or(0)
}

/// Cycles per millisecond of process CPU over the whole run: converts the exact
/// per-frame cycle counts to CPU milliseconds (process times alone move in
/// timer quanta).
pub(super) fn calibration() -> Value {
    let cpu = Snapshot::read().map(|s| s.cpu_100ns).unwrap_or(0);
    let cycles = cycles();
    json!({"process_cycles": cycles, "process_cpu_100ns": cpu,
        "cycles_per_cpu_ms": if cpu > 0 { Some(cycles as f64 / (cpu as f64 / 10_000.)) } else { None }})
}

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
    pub(super) fn read() -> Result<Self, String> {
        let sample = kamin_profiler::process::read_counters();
        let value = |field: kamin_profiler::process::Field<u64>| {
            field.value.ok_or_else(|| {
                field
                    .unavailable
                    .unwrap_or_else(|| "process counter unavailable".into())
            })
        };
        Ok(Self {
            allocations: super::allocations::Snapshot::read(),
            cpu_100ns: value(sample.cpu_100ns)?,
            rss: value(sample.working_set_bytes)?,
            commit: value(sample.private_commit_bytes)?,
            peak_rss: value(sample.lifetime_peak_working_set_bytes)?,
            peak_commit: value(sample.lifetime_peak_private_commit_bytes)?,
        })
    }
    pub(super) fn json(self) -> Value {
        json!({"rust_allocations": self.allocations.json(),
            "rss_bytes": self.rss, "private_commit_bytes": self.commit,
            "process_lifetime_peak_rss_bytes": self.peak_rss,
            "process_lifetime_peak_commit_bytes": self.peak_commit})
    }
}
