//! External CPU checkpoints avoid charging the target for its own observer.

use super::activity::Activity;
use super::{Instant, Value, json, ms};
use gpui::profiler::journal::ForegroundJournalEntry as Entry;

/// Raw process-wide Windows counters over an externally observed quiet span.
/// No rounding of nonzero cycles or CPU ticks is permitted.
pub struct IdleCpuWindow {
    raw: Value,
    duration_ms: f64,
    cycles: u64,
    kernel: u64,
    user: u64,
}

impl IdleCpuWindow {
    pub fn from_json(raw: Value, pid: u32, creation: u64) -> Result<Self, &'static str> {
        let number = |key: &str| raw[key].as_u64().ok_or("missing raw process counter");
        if number("pid")? != u64::from(pid) || number("creation_100ns")? != creation {
            return Err("process identity changed");
        }
        let delta = |key: &str| -> Result<u64, &'static str> {
            raw["end"][key]
                .as_u64()
                .and_then(|end| {
                    raw["start"][key]
                        .as_u64()
                        .and_then(|start| end.checked_sub(start))
                })
                .ok_or("missing or regressing process counter")
        };
        let frequency = number("qpc_frequency")?;
        if frequency == 0 {
            return Err("zero process observation clock frequency");
        }
        let duration_ms = delta("qpc")? as f64 * 1000. / frequency as f64;
        if duration_ms < 5000. {
            return Err("idle CPU observation is shorter than five seconds");
        }
        let cycles = delta("cycles")?;
        let kernel = delta("kernel_100ns")?;
        let user = delta("user_100ns")?;
        Ok(Self {
            raw,
            duration_ms,
            cycles,
            kernel,
            user,
        })
    }
}

pub(super) struct Observation {
    start: Instant,
    activity: Activity,
}

impl Observation {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            activity: Activity::default(),
        }
    }

    pub fn lost(&mut self, count: u64) {
        self.activity.lost(count);
    }

    pub fn observe(&mut self, entry: Entry, window: gpui::WindowId) {
        self.activity.observe(entry, window);
    }

    pub fn finish(self, cpu: IdleCpuWindow) -> Result<Value, &'static str> {
        let observed_ms = ms(self.start.elapsed());
        if observed_ms < cpu.duration_ms {
            return Err("CPU span exceeds journal idle observation");
        }
        let mut result = self.activity.to_json();
        let fields = json!({
            "journal_duration_ms": observed_ms, "cpu_duration_ms": cpu.duration_ms,
            "cpu_raw": cpu.raw, "process_cycles": cpu.cycles,
            "process_kernel_100ns": cpu.kernel, "process_user_100ns": cpu.user,
            "quiet": self.activity.quiet() && cpu.cycles == 0 && cpu.kernel == 0 && cpu.user == 0,
        });
        result
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Value {
        json!({"pid": 7, "creation_100ns": 13, "qpc_frequency": 1000,
            "start": {"qpc": 100, "cycles": 17, "kernel_100ns": 20, "user_100ns": 30},
            "end": {"qpc": 5100, "cycles": 17, "kernel_100ns": 20, "user_100ns": 30}})
    }

    #[test]
    fn raw_idle_counters_require_identity_monotonicity_and_five_seconds() {
        assert!(IdleCpuWindow::from_json(raw(), 7, 13).is_ok());
        assert!(IdleCpuWindow::from_json(raw(), 8, 13).is_err());
        assert!(IdleCpuWindow::from_json(raw(), 7, 14).is_err());
        for (field, value) in [
            ("qpc", 5099),
            ("cycles", 16),
            ("kernel_100ns", 19),
            ("user_100ns", 29),
        ] {
            let mut sample = raw();
            sample["end"][field] = json!(value);
            assert!(IdleCpuWindow::from_json(sample, 7, 13).is_err());
        }
        let mut sample = raw();
        sample["qpc_frequency"] = json!(0);
        assert!(IdleCpuWindow::from_json(sample, 7, 13).is_err());
    }

    #[test]
    fn a_single_raw_cycle_is_never_reported_as_idle_zero() {
        let mut raw = raw();
        raw["end"]["cycles"] = json!(18);
        let sample = IdleCpuWindow::from_json(raw, 7, 13).unwrap();
        let mut observation = Observation::new();
        observation.start -= std::time::Duration::from_secs(6);
        let result = observation.finish(sample).unwrap();
        assert_eq!(result["process_cycles"], 1);
        assert_eq!(result["quiet"], false);
    }

    #[test]
    fn missing_entries_cannot_prove_idle_and_a_short_journal_span_is_rejected() {
        assert!(
            Observation::new()
                .finish(IdleCpuWindow::from_json(raw(), 7, 13).unwrap())
                .is_err()
        );
        let mut observation = Observation::new();
        observation.start -= std::time::Duration::from_secs(6);
        observation.lost(1);
        let result = observation
            .finish(IdleCpuWindow::from_json(raw(), 7, 13).unwrap())
            .unwrap();
        assert_eq!(result["foreground_entries_lost"], 1);
        assert_eq!(result["quiet"], false);
    }
}
