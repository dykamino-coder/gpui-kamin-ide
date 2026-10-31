//! Потоковый сборщик целых счётчиков: одна сессия кадра, исключающие друг друга фазы.

use std::{
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
};
use tracing::{
    Dispatch, Subscriber,
    dispatcher::{DefaultGuard, get_default, set_default},
    span::{Attributes, Id, Record},
};

macro_rules! vocabulary {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug)]
        #[repr(usize)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const NAMES: &[&str] = &[$($label),+];
            pub const ALL: &[Self] = &[$(Self::$variant),+];
            pub const COUNT: usize = Self::NAMES.len();
        }
    };
}

vocabulary!(Counter {
    TextPositionQueries => "text_position_queries",
    TextPositionSteps => "text_position_steps",
    TextPositionBuildBytes => "text_position_build_bytes",
    MulticolProbes => "multicol_probes",
    MulticolChildLayouts => "multicol_child_layouts",
    MulticolTrialRestores => "multicol_trial_restores",
    MeasureCacheHits => "measure_cache_hits",
    MeasureCacheMisses => "measure_cache_misses",
    LayoutCacheHits => "layout_cache_hits",
    LayoutCacheMisses => "layout_cache_misses",
    PaintCacheHits => "paint_cache_hits",
    PaintCacheMisses => "paint_cache_misses",
    ResourceCacheHits => "resource_cache_hits",
    ResourceCacheMisses => "resource_cache_misses",
    RebuiltNodes => "rebuilt_nodes",
    FullTreeWalks => "full_tree_walks",
    TreeNodesVisited => "tree_nodes_visited",
    SubtreeQueries => "subtree_queries",
    SubtreeNodesVisited => "subtree_nodes_visited",
    StyleMatchAttempts => "style_match_attempts",
});
vocabulary!(Phase {
    HostResources => "host_resources", Parse => "parse", Style => "style",
    DocumentOther => "document_other", BuildElements => "build_elements",
    LayoutRequest => "layout_request", Layout => "layout", Prepaint => "prepaint",
    Paint => "paint", RasterGpuReadback => "raster_gpu_readback",
    PipelineOther => "pipeline_other",
});

/// Только работа текущего потока между begin и finish, без диагностических обходов.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot(pub [[u64; Counter::COUNT]; Phase::COUNT]);
impl Default for Snapshot {
    fn default() -> Self {
        Self([[0; Counter::COUNT]; Phase::COUNT])
    }
}
/// Владелец — явно открытая диагностическая сессия, не документ или layout.
struct Totals {
    phase: AtomicUsize,
    counts: [[AtomicU64; Counter::COUNT]; Phase::COUNT],
    observer: Option<Arc<dyn Fn(Phase) + Send + Sync>>,
}
struct Collector(Arc<Totals>);
// Dispatch хранит область наблюдения и восстанавливает предыдущего подписчика.
// Обычные trace-события не нужны: hooks адресуют типизированные счётчики напрямую.
impl Subscriber for Collector {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        false
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &tracing::Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}
fn current<T>(mut f: impl FnMut(&Totals) -> T) -> Option<T> {
    get_default(|dispatch| dispatch.downcast_ref::<Collector>().map(|c| f(&c.0)))
}
/// Сессия не переносится между потоками; вложенная сессия не загрязняет внешнюю.
pub struct Session {
    _scope: DefaultGuard,
    totals: Arc<Totals>,
    thread: PhantomData<Rc<()>>,
}
impl Session {
    pub fn begin() -> Self {
        Self::begin_observed(None)
    }
    /// Наблюдатель границ для стенда; вызывается только при смене фазы.
    pub fn begin_observed(observer: Option<Arc<dyn Fn(Phase) + Send + Sync>>) -> Self {
        let totals = Arc::new(Totals {
            phase: AtomicUsize::new(Phase::PipelineOther as usize),
            counts: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
            observer,
        });
        let dispatch = Dispatch::new(Collector(totals.clone()));
        Self {
            _scope: set_default(&dispatch),
            totals,
            thread: PhantomData,
        }
    }
    pub fn finish(self) -> Snapshot {
        Snapshot(std::array::from_fn(|phase| {
            std::array::from_fn(|counter| self.totals.counts[phase][counter].load(Relaxed))
        }))
    }
}
/// Маркер исключительной фазы; unwind также восстанавливает атрибуцию.
pub struct PhaseGuard {
    totals: Option<Arc<Totals>>,
    previous: usize,
    thread: PhantomData<Rc<()>>,
}
pub fn enter(phase: Phase) -> PhaseGuard {
    let totals = get_default(|dispatch| dispatch.downcast_ref::<Collector>().map(|c| c.0.clone()));
    let previous = totals.as_ref().map_or(0, |s| change_phase(s, phase));
    PhaseGuard {
        totals,
        previous,
        thread: PhantomData,
    }
}
/// Граница между lifecycle callbacks, где RAII-области уже завершились.
pub fn set_phase(phase: Phase) {
    let _ = current(|s| change_phase(s, phase));
}
fn change_phase(totals: &Totals, phase: Phase) -> usize {
    if let Some(observer) = &totals.observer {
        observer(phase);
    }
    totals.phase.swap(phase as usize, Relaxed)
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        if let Some(totals) = &self.totals {
            // Предыдущий индекс получен из того же закрытого словаря фаз.
            let phase = Phase::ALL[self.previous];
            change_phase(totals, phase);
        }
    }
}
pub fn increment(counter: Counter, amount: u64) {
    let _ = current(|s| {
        s.counts[s.phase.load(Relaxed)][counter as usize].fetch_add(amount, Relaxed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_observer_sees_nested_phase_restore() {
        let boundaries = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = boundaries.clone();
        let session = Session::begin_observed(Some(Arc::new(move |phase| {
            observed.lock().unwrap().push(phase as usize);
        })));
        {
            let _layout = enter(Phase::Layout);
            let _paint = enter(Phase::Paint);
        }
        session.finish();
        assert_eq!(
            *boundaries.lock().unwrap(),
            [
                Phase::Layout as usize,
                Phase::Paint as usize,
                Phase::Layout as usize,
                Phase::PipelineOther as usize,
            ]
        );
    }
    #[test]
    fn phases_sessions_and_threads_do_not_overlap() {
        let outer = Session::begin();
        increment(Counter::RebuiltNodes, 1);
        {
            let _paint = enter(Phase::Paint);
            increment(Counter::RebuiltNodes, 2);
            let inner = Session::begin();
            increment(Counter::RebuiltNodes, 99);
            assert_eq!(
                inner.finish().0[Phase::PipelineOther as usize][Counter::RebuiltNodes as usize],
                99
            );
            std::thread::spawn(|| increment(Counter::RebuiltNodes, 100))
                .join()
                .unwrap();
            increment(Counter::RebuiltNodes, 3);
        }
        let snapshot = outer.finish();
        assert_eq!(
            snapshot.0[Phase::Paint as usize][Counter::RebuiltNodes as usize],
            5
        );
        assert_eq!(
            snapshot
                .0
                .iter()
                .map(|r| r[Counter::RebuiltNodes as usize])
                .sum::<u64>(),
            6
        );
    }
    #[test]
    fn aborted_capture_restores_collector() {
        let outer = Session::begin();
        let _ = std::panic::catch_unwind(|| {
            let _session = Session::begin();
            let _phase = enter(Phase::Paint);
            increment(Counter::MulticolProbes, 9);
            panic!("abort sample");
        });
        increment(Counter::MulticolProbes, 1);
        assert_eq!(
            outer.finish().0[Phase::PipelineOther as usize][Counter::MulticolProbes as usize],
            1
        );
    }
}
