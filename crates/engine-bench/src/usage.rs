//! Ресурсы кадра: CPU между границами фаз и выборочные пики памяти через 5 мс.

use super::process::{Snapshot, gpu::Memory};
use kamin_html::work::Phase;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering::SeqCst},
    },
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
/// Чтение процесса и GPU. Делается без замка [`State`]: медленные вызовы ОС
/// (память процесса, память GPU) не держат ни поток сэмплов, ни границу фазы.
struct Reading {
    now: Result<Snapshot, String>,
    gpu: Option<(u64, u64)>,
    gpu_error: Option<String>,
}
impl Reading {
    fn take(gpu: Option<&Memory>) -> Self {
        let now = Snapshot::read().map_err(|error| error.to_string());
        let (gpu, gpu_error) = match gpu.map(Memory::read) {
            Some(Ok(sample)) => (Some(sample), None),
            Some(Err(error)) => (None, Some(error.to_string())),
            None => (None, None),
        };
        Self {
            now,
            gpu,
            gpu_error,
        }
    }
}
struct State {
    phases: [PhaseUsage; Phase::COUNT],
    phase: usize,
    start: Snapshot,
    last: Snapshot,
    gpu_error: Option<String>,
    error: Option<String>,
    stopped: bool,
}
impl State {
    fn apply(&mut self, reading: Reading, next: Option<Phase>) {
        if let Some(error) = reading.gpu_error {
            self.gpu_error = Some(error);
        }
        let now = match reading.now {
            Ok(now) => now,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let gpu = reading.gpu;
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
            #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
            super::allocations::probe::phase(self.phase);
            self.phases[self.phase].observe(now, gpu);
        }
    }
}
/// Граница фазы читает процесс до замка; сэмпл, чтение которого пересеклось с
/// границей, отбрасывается: иначе более раннее чтение легло бы после более
/// позднего, и счётчики фазы ушли бы в минус. Нечётное значение — граница
/// читает процесс, чётное — покой.
struct Boundaries(AtomicU64);
impl Boundaries {
    fn begin(&self) {
        self.0.fetch_add(1, SeqCst);
    }
    fn end(&self) {
        self.0.fetch_add(1, SeqCst);
    }
    fn quiet(&self) -> Option<u64> {
        let epoch = self.0.load(SeqCst);
        epoch.is_multiple_of(2).then_some(epoch)
    }
    fn unchanged(&self, epoch: u64) -> bool {
        self.0.load(SeqCst) == epoch
    }
}
/// Снимок на границе фазы: поток окна, своя копия адаптера GPU (замок
/// только у этого потока, без соперника), запись под коротким замком State.
struct Boundary {
    state: Arc<Mutex<State>>,
    gpu: Mutex<Option<Memory>>,
    boundaries: Arc<Boundaries>,
}
impl Boundary {
    fn observe(&self, next: Option<Phase>) {
        self.boundaries.begin();
        let reading = {
            let gpu = self.gpu.lock().unwrap_or_else(|e| e.into_inner());
            Reading::take(gpu.as_ref())
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.apply(reading, next);
        self.boundaries.end();
    }
}
pub(super) struct Capture {
    boundary: Arc<Boundary>,
    worker: Option<JoinHandle<()>>,
}
impl Capture {
    pub(super) fn begin(luid: (i32, u32)) -> Result<Self, Box<dyn std::error::Error>> {
        let start = Snapshot::read()?;
        #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
        super::allocations::probe::frame();
        let open = || Memory::open(luid.0, luid.1);
        let (sampler_gpu, gpu_error) = match open() {
            Ok(gpu) => (Some(gpu), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let boundary_gpu = sampler_gpu.as_ref().and_then(|_| open().ok());
        let state = Arc::new(Mutex::new(State {
            phases: std::array::from_fn(|_| PhaseUsage::default()),
            phase: Phase::PipelineOther as usize,
            start,
            last: start,
            gpu_error,
            error: None,
            stopped: false,
        }));
        let boundaries = Arc::new(Boundaries(AtomicU64::new(0)));
        let (shared, epochs) = (state.clone(), boundaries.clone());
        let worker = std::thread::spawn(move || {
            loop {
                std::thread::park_timeout(Duration::from_millis(5));
                let Some(epoch) = epochs.quiet() else {
                    continue;
                };
                let reading = Reading::take(sampler_gpu.as_ref());
                let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                if state.stopped {
                    break;
                }
                if epochs.unchanged(epoch) {
                    state.apply(reading, None);
                }
            }
        });
        Ok(Self {
            boundary: Arc::new(Boundary {
                state,
                gpu: Mutex::new(boundary_gpu),
                boundaries,
            }),
            worker: Some(worker),
        })
    }
    pub(super) fn observer(&self) -> Arc<dyn Fn(Phase) + Send + Sync> {
        let boundary = self.boundary.clone();
        Arc::new(move |next| boundary.observe(Some(next)))
    }
    pub(super) fn finish(mut self, cold: bool) -> Value {
        self.stop();
        let state = self
            .boundary
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
                    "rust_allocation_calls": cfg!(feature = "allocation-audit").then_some(usage.allocation_calls),
                    "rust_allocated_bytes": cfg!(feature = "allocation-audit").then_some(usage.allocated_bytes),
                    "rust_freed_bytes": cfg!(feature = "allocation-audit").then_some(usage.freed_bytes),
                    "sampled_peak_rss_bytes": usage.rss,
                    "sampled_peak_private_commit_bytes": usage.commit,
                    "end": usage.end.json(),
                    "sampled_peak_gpu": gpu(usage.gpu_peak), "end_gpu": gpu(usage.gpu_end)})
                    },
                )
            })
            .collect();
        json!({"phases": phases,
            "rust_allocation_audit_enabled": cfg!(feature = "allocation-audit"),
            "process_cpu_100ns": state.last.cpu_100ns - state.start.cpu_100ns,
            "start": state.start.json(), "end": state.last.json(),
            "error": state.error, "gpu_error": state.gpu_error,
            "memory_peak_method": "5ms samples plus phase boundaries; lifetime high-water marks separate; a sample whose read overlaps a boundary is dropped",
            "cpu_scope": "kernel+user, all process threads, including measurement thread; exclusive wall-phase attribution",
            "time_scope": "phase windows exclude the boundary snapshot (kamin_html::work::Clock); it is counted in pipeline_other",
            "steady_method": "end of phase/frame; not a post-GC/quiescence assertion"})
    }
    fn stop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.boundary.observe(None);
            self.boundary
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .stopped = true;
            #[cfg(any(feature = "allocation-audit", feature = "heap-profile"))]
            super::allocations::probe::outside();
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
mod tests;
