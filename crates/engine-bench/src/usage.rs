//! Ресурсы кадра: CPU между границами фаз и выборочные пики памяти через 5 мс.

use super::process::{Snapshot, gpu::Memory};
use kamin_html::work::Phase;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

#[derive(Clone, Default)]
struct PhaseUsage {
    allocation_calls: u64,
    allocated_bytes: u64,
    freed_bytes: u64,
    cpu: u64,
    rss: u64,
    commit: u64,
    end: Snapshot,
    gpu_peak: Option<(u64, u64)>,
    gpu_end: Option<(u64, u64)>,
}
impl PhaseUsage {
    fn observe(&mut self, now: Snapshot, gpu: Option<(u64, u64)>) {
        self.rss = self.rss.max(now.rss);
        self.commit = self.commit.max(now.commit);
        self.end = now;
        if let Some((local, shared)) = gpu {
            let peak = self.gpu_peak.get_or_insert((0, 0));
            peak.0 = peak.0.max(local);
            peak.1 = peak.1.max(shared);
            self.gpu_end = gpu;
        }
    }
}
struct State {
    phases: [PhaseUsage; Phase::COUNT],
    phase: usize,
    start: Snapshot,
    last: Snapshot,
    gpu: Option<Memory>,
    gpu_error: Option<String>,
    error: Option<String>,
    stopped: bool,
}
impl State {
    fn observe(&mut self, next: Option<Phase>) {
        match Snapshot::read() {
            Ok(now) => {
                let gpu = if let Some(gpu) = &self.gpu {
                    match gpu.read() {
                        Ok(sample) => Some(sample),
                        Err(error) => {
                            self.gpu_error = Some(error.to_string());
                            None
                        }
                    }
                } else {
                    None
                };
                let usage = &mut self.phases[self.phase];
                usage.allocation_calls += now.allocations.calls - self.last.allocations.calls;
                usage.allocated_bytes += now.allocations.bytes - self.last.allocations.bytes;
                usage.freed_bytes += now.allocations.freed - self.last.allocations.freed;
                if let Some(delta) = now.cpu_100ns.checked_sub(self.last.cpu_100ns) {
                    usage.cpu += delta;
                } else {
                    self.error = Some("process CPU clock regressed".into());
                }
                usage.observe(now, gpu);
                self.last = now;
                if let Some(next) = next {
                    self.phase = next as usize;
                    self.phases[self.phase].observe(now, gpu);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}
pub(super) struct Capture {
    state: Arc<Mutex<State>>,
    worker: Option<JoinHandle<()>>,
}
impl Capture {
    pub(super) fn begin(luid: (i32, u32)) -> Result<Self, Box<dyn std::error::Error>> {
        let start = Snapshot::read()?;
        let (gpu, gpu_error) = match Memory::open(luid.0, luid.1) {
            Ok(gpu) => (Some(gpu), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let state = Arc::new(Mutex::new(State {
            phases: std::array::from_fn(|_| PhaseUsage::default()),
            phase: Phase::PipelineOther as usize,
            start,
            last: start,
            gpu,
            gpu_error,
            error: None,
            stopped: false,
        }));
        let shared = state.clone();
        let worker = std::thread::spawn(move || {
            loop {
                std::thread::park_timeout(Duration::from_millis(5));
                let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                if state.stopped {
                    break;
                }
                state.observe(None);
            }
        });
        Ok(Self {
            state,
            worker: Some(worker),
        })
    }
    pub(super) fn observer(&self) -> Arc<dyn Fn(Phase) + Send + Sync> {
        let state = self.state.clone();
        Arc::new(move |next| {
            state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .observe(Some(next))
        })
    }
    pub(super) fn finish(mut self, cold: bool) -> Value {
        self.stop();
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let phases: serde_json::Map<String, Value> = Phase::NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let usage = &state.phases[i];
                let gpu = |v: Option<(u64, u64)>| {
                    v.map(
                        |(local, shared)| json!({"local_bytes": local, "non_local_bytes": shared}),
                    )
                };
                (
                    (*name).into(),
                    if !cold && i <= Phase::DocumentOther as usize {
                        Value::Null
                    } else {
                        json!({"process_cpu_100ns": usage.cpu,
                    "rust_allocation_calls": usage.allocation_calls,
                    "rust_allocated_bytes": usage.allocated_bytes,
                    "rust_freed_bytes": usage.freed_bytes,
                    "sampled_peak_rss_bytes": usage.rss,
                    "sampled_peak_private_commit_bytes": usage.commit,
                    "end": usage.end.json(),
                    "sampled_peak_gpu": gpu(usage.gpu_peak), "end_gpu": gpu(usage.gpu_end)})
                    },
                )
            })
            .collect();
        json!({"phases": phases,
            "process_cpu_100ns": state.last.cpu_100ns - state.start.cpu_100ns,
            "start": state.start.json(), "end": state.last.json(),
            "error": state.error, "gpu_error": state.gpu_error,
            "memory_peak_method": "5ms samples plus phase boundaries; lifetime high-water marks separate",
            "cpu_scope": "kernel+user, all process threads, including measurement thread; exclusive wall-phase attribution",
            "steady_method": "end of phase/frame; not a post-GC/quiescence assertion"})
    }
    fn stop(&mut self) {
        if let Some(worker) = self.worker.take() {
            {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                state.observe(None);
                state.stopped = true;
            }
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_samples_preserve_peak_and_distinguish_end_memory() {
        let mut phase = PhaseUsage::default();
        phase.observe(
            Snapshot {
                rss: 100,
                commit: 300,
                ..Default::default()
            },
            Some((80, 40)),
        );
        phase.observe(
            Snapshot {
                rss: 50,
                commit: 400,
                ..Default::default()
            },
            Some((60, 70)),
        );
        assert_eq!((phase.rss, phase.commit), (100, 400));
        assert_eq!((phase.end.rss, phase.end.commit), (50, 400));
        assert_eq!(phase.gpu_peak, Some((80, 70)));
        assert_eq!(phase.gpu_end, Some((60, 70)));
    }
}
