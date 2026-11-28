//! Shared ownership of the existing profiler trace subscription.

use super::{Ordering, TRACE_SCOPE_COUNT_MASK, TRACE_STATE, clear_trace_buffers};

/// Keeps profiler observations enabled until the last scoped reader drops.
pub struct TraceGuard;

/// Subscribe without disabling another profiler reader when this guard drops.
pub fn trace_scope() -> TraceGuard {
    let incremented = TRACE_STATE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
        (state & TRACE_SCOPE_COUNT_MASK < TRACE_SCOPE_COUNT_MASK).then_some(state + 1)
    });
    assert!(incremented.is_ok(), "too many active profiler trace scopes");
    TraceGuard
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        let previous_state =
            TRACE_STATE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state & TRACE_SCOPE_COUNT_MASK > 0).then_some(state - 1)
            });
        match previous_state {
            Ok(1) => clear_trace_buffers(),
            Ok(_) => {}
            Err(_) => debug_assert!(false, "profiler trace scope count underflowed"),
        }
    }
}
