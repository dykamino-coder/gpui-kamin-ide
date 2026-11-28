//! Интервал в фазе входа; без активной сессии часы не читаются.

use super::{Collector, Counter, Relaxed, Totals, get_default};
use std::{marker::PhantomData, rc::Rc, sync::Arc, time::Instant};

pub struct Timer {
    start: Option<(Instant, Arc<Totals>, usize)>,
    counter: Counter,
    allocations: Option<(u64, u64)>,
    thread: PhantomData<Rc<()>>,
}

pub fn time(counter: Counter) -> Timer {
    let start = get_default(|dispatch| {
        dispatch.downcast_ref::<Collector>().map(|c| {
            let phase = c.0.phase.load(Relaxed);
            (Instant::now(), c.0.clone(), phase)
        })
    });
    let allocations = start
        .as_ref()
        .and_then(|(_, totals, _)| totals.allocations.map(|f| f()));
    Timer {
        start,
        counter,
        allocations,
        thread: PhantomData,
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((start, totals, phase)) = &self.start {
            let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            totals.counts[*phase][self.counter as usize].fetch_add(ns, Relaxed);
            let counters = match self.counter {
                Counter::SvgSerializeNs => Some((
                    Counter::SvgSerializeAllocations,
                    Counter::SvgSerializeAllocatedBytes,
                )),
                Counter::SvgParseNs => Some((
                    Counter::SvgParseAllocations,
                    Counter::SvgParseAllocatedBytes,
                )),
                Counter::SvgRenderNs => Some((
                    Counter::SvgRenderAllocations,
                    Counter::SvgRenderAllocatedBytes,
                )),
                _ => None,
            };
            if let (Some(before), Some(sample), Some((calls, bytes))) =
                (self.allocations, totals.allocations, counters)
            {
                let after = sample();
                totals.counts[*phase][calls as usize]
                    .fetch_add(after.0.saturating_sub(before.0), Relaxed);
                totals.counts[*phase][bytes as usize]
                    .fetch_add(after.1.saturating_sub(before.1), Relaxed);
            }
        }
    }
}

/// An interval whose counter gets only its own time: the time of self-timed
/// intervals opened inside it goes to theirs. The self-timed counters of one
/// tree of calls sum to the time of its outermost interval, as a profiler's
/// self time does.
pub struct SelfTimer {
    start: Option<(Instant, Arc<Totals>, usize, u64)>,
    counter: Counter,
    thread: PhantomData<Rc<()>>,
}

pub fn time_self(counter: Counter) -> SelfTimer {
    let start = get_default(|dispatch| {
        dispatch.downcast_ref::<Collector>().map(|c| {
            let outer = c.0.nested_ns.swap(0, Relaxed);
            (Instant::now(), c.0.clone(), c.0.phase.load(Relaxed), outer)
        })
    });
    SelfTimer {
        start,
        counter,
        thread: PhantomData,
    }
}

impl Drop for SelfTimer {
    fn drop(&mut self) {
        if let Some((start, totals, phase, outer)) = &self.start {
            let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let nested = totals.nested_ns.swap(outer.saturating_add(ns), Relaxed);
            totals.counts[*phase][self.counter as usize]
                .fetch_add(ns.saturating_sub(nested), Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Phase, Session, enter};

    #[test]
    fn self_timers_count_only_their_own_time() {
        let session = Session::begin();
        let outer = time_self(Counter::BuildSelfScrollNs);
        {
            let _inner = time_self(Counter::BuildSelfElementNs);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(outer);
        let snapshot = session.finish();
        let at = |c: Counter| snapshot.0[Phase::PipelineOther as usize][c as usize];
        let (outer, inner) = (
            at(Counter::BuildSelfScrollNs),
            at(Counter::BuildSelfElementNs),
        );
        assert!(inner >= 20_000_000, "{inner}");
        assert!(
            outer < inner / 2,
            "the outer interval counted the inner one: {outer}"
        );
    }
    use super::*;

    #[test]
    fn allocation_sample_keeps_entry_phase_and_session() {
        fn sample() -> (u64, u64) {
            static SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = SAMPLES.fetch_add(1, Relaxed);
            (n * 3, n * 48)
        }
        let session = Session::begin_measured(None, Some(sample));
        let layout = enter(Phase::Layout);
        let timer = time(Counter::SvgParseNs);
        let paint = enter(Phase::Paint);
        let inner = Session::begin_measured(None, Some(|| (1000, 2000)));
        drop(timer);
        let nested = inner.finish();
        drop(paint);
        drop(layout);
        let snapshot = session.finish();
        assert_eq!(
            snapshot.0[Phase::Layout as usize][Counter::SvgParseAllocations as usize],
            3
        );
        assert_eq!(
            snapshot.0[Phase::Layout as usize][Counter::SvgParseAllocatedBytes as usize],
            48
        );
        assert_eq!(
            snapshot.0[Phase::Paint as usize][Counter::SvgParseAllocations as usize],
            0
        );
        assert_eq!(
            nested.0[Phase::PipelineOther as usize][Counter::SvgParseAllocations as usize],
            0
        );
    }

    #[test]
    fn timer_keeps_entry_phase_and_session_across_unwind() {
        assert!(time(Counter::BandProbeBuildNs).start.is_none());
        let session = Session::begin();
        let _ = std::panic::catch_unwind(|| {
            let _layout = enter(Phase::Layout);
            let timer = time(Counter::BandProbeBuildNs);
            assert!(timer.start.is_some());
            let _paint = enter(Phase::Paint);
            let _inner = Session::begin();
            std::hint::black_box((0..1000).sum::<u64>());
            panic!("unwind timer");
        });
        let snapshot = session.finish();
        assert!(snapshot.0[Phase::Layout as usize][Counter::BandProbeBuildNs as usize] > 0);
        assert_eq!(
            snapshot.0[Phase::Paint as usize][Counter::BandProbeBuildNs as usize],
            0
        );
    }
}
