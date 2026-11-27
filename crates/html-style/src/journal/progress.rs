//! Bounded, non-consuming journal snapshots for a reader outside a stalled pass.

use super::{Journal, Shared};
use std::time::{Duration, Instant};

const LIMIT: usize = 256;
static ACTIVE: Shared<State> = Shared::new(State {
    next: 0,
    spans: Vec::new(),
    dropped: 0,
});

struct State {
    next: u64,
    spans: Vec<(u64, &'static str, Instant)>,
    dropped: u64,
}

/// An interval which has begun but has not returned yet.
#[derive(Clone, Debug)]
pub struct ActiveSpan {
    pub name: &'static str,
    pub elapsed: Duration,
}

/// Completed work since the last `take`, and currently running intervals.
/// The two snapshots are sampled separately; they are diagnostic, not additive.
pub struct Progress {
    pub journal: Journal,
    pub active: Vec<ActiveSpan>,
    /// Intervals omitted at the capacity limit over this process's lifetime.
    pub active_dropped: u64,
}

/// Read without draining the frame's journal. Safe on an observer thread;
/// taking a snapshot does not subscribe or enable engine instrumentation.
pub fn snapshot() -> Progress {
    let journal = super::JOURNAL.lock().clone();
    let state = ACTIVE.lock();
    Progress {
        journal,
        active: state
            .spans
            .iter()
            .map(|(_, name, started)| ActiveSpan {
                name,
                elapsed: started.elapsed(),
            })
            .collect(),
        active_dropped: state.dropped,
    }
}

pub(super) fn begin(name: &'static str, started: Instant) -> Option<u64> {
    let mut state = ACTIVE.lock();
    if state.spans.len() == LIMIT {
        state.dropped += 1;
        return None;
    }
    let id = state.next;
    state.next = state.next.wrapping_add(1);
    state.spans.push((id, name, started));
    Some(id)
}

pub(super) fn end(id: u64) {
    let mut state = ACTIVE.lock();
    if let Some(index) = state.spans.iter().position(|(key, _, _)| *key == id) {
        state.spans.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{Span, take, test_reader};

    #[test]
    fn observer_sees_running_pass_and_does_not_consume_completed_work() {
        let (_one, _reader) = test_reader();
        let pass = Span::new("layout.pass");
        drop(Span::new("layout.child"));
        std::thread::spawn(|| {
            let progress = snapshot();
            assert!(progress.active.iter().any(|s| s.name == "layout.pass"));
            assert_eq!(progress.journal.timings["layout.child"].calls, 1);
        })
        .join()
        .unwrap();
        assert_eq!(take().timings["layout.child"].calls, 1);
        assert!(snapshot().active.iter().any(|s| s.name == "layout.pass"));
        drop(pass);
        assert!(snapshot().active.is_empty());
        assert_eq!(take().timings["layout.pass"].calls, 1);
    }

    #[test]
    fn active_intervals_are_bounded_and_release_their_slots() {
        let (_one, _reader) = test_reader();
        let before = snapshot().active_dropped;
        let spans: Vec<_> = (0..LIMIT + 3).map(|_| Span::new("nested")).collect();
        let progress = snapshot();
        assert_eq!(progress.active.len(), LIMIT);
        assert_eq!(progress.active_dropped - before, 3);
        drop(spans);
        assert!(snapshot().active.is_empty());
    }
}
