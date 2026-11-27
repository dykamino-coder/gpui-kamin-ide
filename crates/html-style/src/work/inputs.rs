//! Layout units whose inputs repeat: a unit (a node and every input of its
//! layout, hashed by the caller) already laid out earlier in this frame or in
//! the previous frame of the session chain. Their time is the upper bound of
//! a cache of layout results keyed by inputs: what such a cache would have
//! returned instead of laying the unit out again. Only the outermost
//! repeated unit is timed, so nested repeats are not counted twice.

use super::{Collector, Counter, Relaxed, Totals, get_default};
use std::{collections::HashSet, marker::PhantomData, rc::Rc, sync::Arc, time::Instant};

/// Keys of this frame, keys of the previous one, open timed repeats.
#[derive(Default)]
pub(super) struct Inputs {
    now: HashSet<u64>,
    last: HashSet<u64>,
    open: usize,
}

/// The keys of a finished frame, handed to the session of the next one.
#[derive(Clone, Debug, Default)]
pub struct Carry(HashSet<u64>);

impl Carry {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl super::Session {
    /// The previous frame's keys, before the first unit of this one.
    pub fn resume_inputs(&self, carry: Carry) {
        if let Ok(mut inputs) = self.totals.inputs.lock() {
            inputs.last = carry.0;
        }
    }
    /// The keys of this frame, for the next session.
    pub fn carry_inputs(&self) -> Carry {
        self.totals
            .inputs
            .lock()
            .map(|mut inputs| Carry(std::mem::take(&mut inputs.now)))
            .unwrap_or_default()
    }
}

/// Times the unit while it lives when it is the outermost repeat.
pub struct Repeat {
    start: Option<(Instant, Arc<Totals>, usize, Counter)>,
    thread: PhantomData<Rc<()>>,
}

/// Note one layout unit with inputs `key`; without a session nothing is read.
pub fn repeat(key: u64) -> Repeat {
    let start = get_default(|dispatch| {
        let totals = dispatch.downcast_ref::<Collector>()?.0.clone();
        let phase = totals.phase.load(Relaxed);
        let add = |c: Counter| totals.counts[phase][c as usize].fetch_add(1, Relaxed);
        add(Counter::LayoutUnits);
        let mut inputs = totals.inputs.lock().ok()?;
        let (count, ns) = if !inputs.now.insert(key) {
            (
                Counter::LayoutUnitsRepeatedInFrame,
                Counter::LayoutRepeatedInFrameNs,
            )
        } else if inputs.last.contains(&key) {
            (
                Counter::LayoutUnitsSameAsLastFrame,
                Counter::LayoutSameAsLastFrameNs,
            )
        } else {
            return None;
        };
        add(count);
        if inputs.open > 0 {
            return None;
        }
        inputs.open += 1;
        drop(inputs);
        Some((Instant::now(), totals, phase, ns))
    });
    Repeat {
        start,
        thread: PhantomData,
    }
}

impl Drop for Repeat {
    fn drop(&mut self) {
        if let Some((start, totals, phase, counter)) = &self.start {
            let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            totals.counts[*phase][*counter as usize].fetch_add(ns, Relaxed);
            if let Ok(mut inputs) = totals.inputs.lock() {
                inputs.open -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Phase, Session, enter};
    use super::*;

    fn at(s: &super::super::Snapshot, c: Counter) -> u64 {
        s.0[Phase::Layout as usize][c as usize]
    }

    #[test]
    fn repeats_are_counted_by_frame_and_only_the_outermost_is_timed() {
        let first = Session::begin();
        {
            let _layout = enter(Phase::Layout);
            drop(repeat(1));
            drop(repeat(2));
            let outer = repeat(1);
            drop(repeat(2));
            drop(outer);
        }
        let carry = first.carry_inputs();
        let one = first.finish();
        assert_eq!(at(&one, Counter::LayoutUnits), 4);
        assert_eq!(at(&one, Counter::LayoutUnitsRepeatedInFrame), 2);
        assert_eq!(at(&one, Counter::LayoutUnitsSameAsLastFrame), 0);
        assert_eq!(carry.len(), 2);
        let second = Session::begin();
        second.resume_inputs(carry);
        {
            let _layout = enter(Phase::Layout);
            let outer = repeat(2);
            std::hint::black_box((0..1000).sum::<u64>());
            drop(repeat(3));
            drop(outer);
        }
        let two = second.finish();
        assert_eq!(at(&two, Counter::LayoutUnitsSameAsLastFrame), 1);
        assert_eq!(at(&two, Counter::LayoutUnitsRepeatedInFrame), 0);
        assert!(at(&two, Counter::LayoutSameAsLastFrameNs) > 0);
    }

    #[test]
    fn without_a_session_nothing_is_kept() {
        assert!(repeat(7).start.is_none());
        let session = Session::begin();
        assert!(session.carry_inputs().is_empty());
    }
}
