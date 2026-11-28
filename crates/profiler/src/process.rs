//! Process counters with independent availability and an explicit sample interval.
//! Creation/polling is opt-in; merely linking the service starts no timers or work.

use serde::Serialize;
use std::time::Instant;

mod gpu_load;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Memory as GpuMemory;

/// Read cumulative counters without opening a GPU adapter, timer or rate query.
/// Sequence and intervals are assigned only by [`Poller`].
pub fn read_counters() -> Sample {
    #[cfg(windows)]
    return windows::sample();
    #[cfg(not(windows))]
    unsupported()
}

/// The cycle counter alone, for precise boundaries without a memory query.
pub fn cycles() -> Field<u64> {
    #[cfg(windows)]
    return windows::cycles();
    #[cfg(not(windows))]
    Field::unavailable("process cycles unsupported on this platform")
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Field<T> {
    pub value: Option<T>,
    pub unavailable: Option<String>,
}

impl<T> Field<T> {
    pub fn available(value: T) -> Self {
        Self {
            value: Some(value),
            unavailable: None,
        }
    }
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            value: None,
            unavailable: Some(reason.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Adapter {
    pub luid_high: i32,
    pub luid_low: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Utilization {
    /// Individual busiest engine on the selected adapter, not a sum of engines.
    pub engine: String,
    pub percent: f64,
    /// Interval between the two successful PDH collections.
    pub interval_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gpu {
    pub adapter: Option<Adapter>,
    pub node: u32,
    pub local_bytes: Field<u64>,
    pub non_local_bytes: Field<u64>,
    pub utilization: Field<Utilization>,
}

impl Gpu {
    fn unavailable(adapter: Option<Adapter>, reason: &str) -> Self {
        Self {
            adapter,
            node: 0,
            local_bytes: Field::unavailable(reason),
            non_local_bytes: Field::unavailable(reason),
            utilization: Field::unavailable(reason),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Sample {
    pub sequence: u64,
    pub elapsed_ns: u64,
    pub interval_ns: Option<u64>,
    pub cpu_100ns: Field<u64>,
    pub cpu_cycles: Field<u64>,
    /// 100 means one fully occupied logical core; may exceed 100.
    pub cpu_core_percent: Field<f64>,
    pub private_commit_bytes: Field<u64>,
    pub working_set_bytes: Field<u64>,
    pub lifetime_peak_private_commit_bytes: Field<u64>,
    pub lifetime_peak_working_set_bytes: Field<u64>,
    pub gpu: Gpu,
}

pub struct Poller {
    origin: Instant,
    previous: Option<(u64, u64)>,
    previous_time: Option<u64>,
    sequence: u64,
    adapter: Option<Adapter>,
    #[cfg(windows)]
    gpu: Option<windows::GpuCounters>,
}

impl Poller {
    /// The adapter must be the renderer's actual LUID. No guessed default GPU.
    pub fn new(adapter: Option<Adapter>) -> Self {
        Self {
            origin: Instant::now(),
            previous: None,
            previous_time: None,
            sequence: 0,
            adapter,
            #[cfg(windows)]
            gpu: adapter.map(windows::GpuCounters::new),
        }
    }

    pub fn poll(&mut self) -> Sample {
        let now = self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.sequence += 1;
        let mut sample = read_counters();
        sample.sequence = self.sequence;
        sample.elapsed_ns = now;
        sample.interval_ns = self.previous_time.map(|time| now.saturating_sub(time));
        self.previous_time = Some(now);
        if let Some((previous_time, previous_cpu)) = self.previous {
            sample.cpu_core_percent = cpu_rate(previous_cpu, &sample.cpu_100ns, previous_time, now);
        }
        // A failed reading breaks the rate interval; the next good sample is a
        // new baseline rather than a delta over a different interval.
        self.previous = sample.cpu_100ns.value.map(|cpu| (now, cpu));
        #[cfg(windows)]
        if let Some(gpu) = self.gpu.as_mut() {
            sample.gpu = gpu.poll();
        }
        if self.adapter.is_none() {
            sample.gpu = Gpu::unavailable(None, "renderer adapter not supplied");
        }
        sample
    }
}

fn cpu_rate(previous: u64, current: &Field<u64>, start_ns: u64, end_ns: u64) -> Field<f64> {
    match (
        current.value.and_then(|value| value.checked_sub(previous)),
        end_ns.checked_sub(start_ns),
    ) {
        (Some(ticks), Some(ns)) if ns > 0 => Field::available(ticks as f64 * 10_000.0 / ns as f64),
        _ => Field::unavailable("CPU counter unavailable, reset, or zero sample interval"),
    }
}

fn unsupported() -> Sample {
    Sample {
        sequence: 0,
        elapsed_ns: 0,
        interval_ns: None,
        cpu_100ns: Field::unavailable("process counters unsupported on this platform"),
        cpu_cycles: Field::unavailable("process counters unsupported on this platform"),
        cpu_core_percent: Field::unavailable("first CPU sample has no rate interval"),
        private_commit_bytes: Field::unavailable("process counters unsupported on this platform"),
        working_set_bytes: Field::unavailable("process counters unsupported on this platform"),
        lifetime_peak_private_commit_bytes: Field::unavailable(
            "process counters unsupported on this platform",
        ),
        lifetime_peak_working_set_bytes: Field::unavailable(
            "process counters unsupported on this platform",
        ),
        gpu: Gpu::unavailable(None, "GPU counters unsupported on this platform"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_rate_uses_wall_interval_and_reports_one_core_without_normalizing() {
        assert_eq!(
            cpu_rate(100, &Field::available(10_000_100), 0, 1_000_000_000).value,
            Some(100.0)
        );
        assert_eq!(
            cpu_rate(0, &Field::available(20_000_000), 0, 1_000_000_000).value,
            Some(200.0)
        );
        assert!(cpu_rate(100, &Field::available(99), 0, 1).value.is_none());
        assert!(cpu_rate(0, &Field::available(10), 1, 1).value.is_none());
    }

    #[test]
    fn unavailable_is_json_null_with_a_reason_not_a_zero_measurement() {
        let sample = serde_json::to_value(unsupported()).unwrap();
        assert!(sample["private_commit_bytes"]["value"].is_null());
        assert!(sample["private_commit_bytes"]["unavailable"].is_string());
        assert!(sample["gpu"]["local_bytes"]["value"].is_null());
    }
}
