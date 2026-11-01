//! Интервал в фазе входа; без активной сессии часы не читаются.

use super::{Collector, Counter, Relaxed, Totals, get_default};
use std::{marker::PhantomData, rc::Rc, sync::Arc, time::Instant};

pub struct Timer {
    start: Option<(Instant, Arc<Totals>, usize)>,
    counter: Counter,
    thread: PhantomData<Rc<()>>,
}

pub fn time(counter: Counter) -> Timer {
    let start = get_default(|dispatch| {
        dispatch.downcast_ref::<Collector>().map(|c| {
            let phase = c.0.phase.load(Relaxed);
            (Instant::now(), c.0.clone(), phase)
        })
    });
    Timer {
        start,
        counter,
        thread: PhantomData,
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((start, totals, phase)) = &self.start {
            let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            totals.counts[*phase][self.counter as usize].fetch_add(ns, Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Phase, Session, enter};
    use super::*;

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
