//! KaminIDE patch (DevTools `why`): bounded frame events shared by diagnostic readers.
use std::panic::Location;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

#[cfg(feature = "test-support")]
mod layout_budget;
#[cfg(feature = "test-support")]
pub use layout_budget::LayoutPassBudget;

/// Events kept between two takes; later ones are only counted.
const EVENTS: usize = 1024;
static READERS: AtomicUsize = AtomicUsize::new(0);
static LOG: Mutex<(Vec<Event>, u64)> = Mutex::new((Vec::new(), 0));

/// A request of a frame (`notify`, `refresh`, `next-frame`, ...) or a
/// mark of the work it caused (`draw`, `render`), at its source line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Stable code of the event.
    pub kind: &'static str,
    /// The source line that requested or marked it.
    pub site: &'static Location<'static>,
    /// What the event saw (scroll nodes: index, id, clip, pairing).
    pub detail: String,
}

/// A record of the whole process kept for the readers of this domain
/// (the engine's journal of causes and cache misses beside these frame
/// events, and the serialization of tests that read them): the
/// observation domain's one kind of lock, taken only while a reader
/// records or reads, never by work that nobody observes.
pub struct Shared<T>(Mutex<T>);

impl<T> Shared<T> {
    /// A record holding `value`.
    pub const fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    /// The record; a reader that panicked leaves it readable.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Recording lasts while a guard lives.
pub struct Trace(());

impl Trace {
    /// Start recording for a new reader.
    pub fn start() -> Self {
        READERS.fetch_add(1, Relaxed);
        Trace(())
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if READERS.fetch_sub(1, Relaxed) == 1 {
            take();
        }
    }
}

/// Whether some reader traces.
#[inline]
pub fn enabled() -> bool {
    READERS.load(Relaxed) > 0
}

/// Record `kind` at `site` while traced.
#[inline]
pub fn push(kind: &'static str, site: &'static Location<'static>) {
    push_with(kind, site, String::new);
}

/// `push` with a detail computed only while traced.
#[inline]
pub fn push_with(kind: &'static str, site: &'static Location<'static>, detail: impl FnOnce() -> String) {
    if enabled() {
        let detail = detail();
        #[cfg(feature = "test-support")]
        layout_budget::observe(kind, &detail);
        let mut log = LOG.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match log.0.len() < EVENTS {
            true => log.0.push(Event { kind, site, detail }),
            false => log.1 += 1,
        }
    }
}

/// `push` of a mark made at the caller (`draw`, `render`).
#[track_caller]
#[inline]
pub fn mark(kind: &'static str) {
    push(kind, Location::caller());
}

/// The events since the last take and how many were past the bound.
pub fn take() -> (Vec<Event>, u64) {
    std::mem::take(&mut *LOG.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Read pending events without consuming the frame owner's trace.
pub fn snapshot() -> (Vec<Event>, u64) {
    LOG.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_in_order_only_while_a_trace_lives() {
        mark("off");
        assert!(take().0.iter().all(|e| e.kind != "off"));
        let trace = Trace::start();
        let line = line!() + 1;
        mark("draw");
        super::super::note("refresh", Location::caller());
        let (events, _) = take();
        drop(trace);
        // Only this test's events: tests running at once record theirs too.
        let mine: Vec<_> = events.iter().filter(|e| e.site.file() == file!() && (e.kind == "draw" || e.kind == "refresh")).collect();
        assert_eq!(mine.iter().map(|e| e.kind).collect::<Vec<_>>(), ["draw", "refresh"]);
        assert_eq!(mine[0].site.line(), line);
        mark("after");
        assert!(take().0.iter().all(|e| e.kind != "after"));
    }
}
