//! Journal of why the engine did work (DevTools "why" domain, schema
//! [`SCHEMA`]): why a frame was built instead of replayed, why a write took
//! the slow path, why a cache lookup missed. Off by default; while off every
//! call is one relaxed load and its arguments are not computed
//! (`detail` closures), like a disabled-by-default trace category.
//!
//! A record carries a stable code (part of the protocol: renamed only with a
//! schema bump), a detail string for people and the call site in the engine
//! source (`#[track_caller]`), so a reader goes from a frame to the line that
//! decided it. Hosts take the journal between frames ([`take`]).

pub use gpui::dirty_sources::trace::Shared;
use std::collections::BTreeMap;
use std::panic::Location;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// Version of the journal's shape and codes for protocol readers.
pub const SCHEMA: u32 = 1;
/// Causes kept between two takes; later ones are only counted.
const CAUSES: usize = 512;
/// Examples kept per cache miss reason between two takes.
const EXAMPLES: usize = 4;

static READERS: AtomicUsize = AtomicUsize::new(0);
/// Readers among `READERS` that take the notes of their own thread only.
static THREAD_READERS: AtomicUsize = AtomicUsize::new(0);
/// The threads of the thread readers, one entry per reader.
static THREADS: Shared<Vec<std::thread::ThreadId>> = Shared::new(Vec::new());
static JOURNAL: Shared<Journal> = Shared::new(Journal::new());

/// Whether the journal records (some reader is subscribed).
#[inline]
pub fn enabled() -> bool {
    READERS.load(Relaxed) > 0
}

/// A reader of the journal: recording lasts while one lives; when the last
/// goes, what was not taken is dropped, with the keys kept to explain misses.
#[must_use = "the journal records only while the subscription lives"]
pub struct Subscription(());

/// Start recording for a new reader.
pub fn subscribe() -> Subscription {
    READERS.fetch_add(1, Relaxed);
    Subscription(())
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if READERS.fetch_sub(1, Relaxed) == 1 {
            take();
            crate::resources::css_images::forget_why();
        }
    }
}

/// A reader of the notes made on its own thread ([`subscribe_thread`]).
#[must_use = "the journal records only while the subscription lives"]
pub struct ThreadSubscription(Subscription, std::marker::PhantomData<*const ()>);

/// Start recording the notes made on the calling thread: while only such
/// readers live, a note from another thread (a test running beside them) is
/// not kept. Any process-wide reader keeps every note.
pub fn subscribe_thread() -> ThreadSubscription {
    THREAD_READERS.fetch_add(1, Relaxed);
    THREADS.lock().push(std::thread::current().id());
    ThreadSubscription(subscribe(), std::marker::PhantomData)
}

impl Drop for ThreadSubscription {
    fn drop(&mut self) {
        let here = std::thread::current().id();
        let mut threads = THREADS.lock();
        if let Some(at) = threads.iter().position(|t| *t == here) {
            threads.swap_remove(at);
        }
        drop(threads);
        THREAD_READERS.fetch_sub(1, Relaxed);
    }
}

/// Whether a note made on this thread is kept: a process-wide reader lives,
/// or a thread reader of this thread.
fn kept_here() -> bool {
    READERS.load(Relaxed) > THREAD_READERS.load(Relaxed)
        || THREADS.lock().contains(&std::thread::current().id())
}

/// One decision: its stable code, details and the engine line that made it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cause {
    pub code: &'static str,
    pub detail: String,
    pub site: &'static Location<'static>,
}

impl Cause {
    /// `file:line` of the deciding call.
    pub fn site(&self) -> String {
        format!(
            "{}:{}",
            self.site.file().replace('\\', "/"),
            self.site.line()
        )
    }
}

/// Lookups of one cache since the last take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    /// Misses by reason code.
    pub misses: BTreeMap<&'static str, u64>,
    /// First misses of each reason, as causes.
    pub examples: Vec<Cause>,
}

/// What was recorded since the last take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Journal {
    pub causes: Vec<Cause>,
    /// Causes past the bound, not kept.
    pub dropped: u64,
    pub caches: BTreeMap<&'static str, CacheStats>,
    pub scroll_animation_frames: u64,
    pub scroll_animation_builds: u64,
    pub window_frames: u64,
    pub window_document_builds: u64,
}

impl Journal {
    const fn new() -> Self {
        Self {
            causes: Vec::new(),
            dropped: 0,
            caches: BTreeMap::new(),
            scroll_animation_frames: 0,
            scroll_animation_builds: 0,
            window_frames: 0,
            window_document_builds: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.causes.is_empty()
            && self.dropped == 0
            && self.caches.is_empty()
            && self.scroll_animation_frames == 0
            && self.window_frames == 0
            && self.window_document_builds == 0
    }
}

mod frames;
pub use frames::{scroll_animation_frame, window_document_build, window_frame};

fn with(f: impl FnOnce(&mut Journal)) {
    f(&mut JOURNAL.lock());
}

/// Record a decision `code` made at the caller; `detail` runs only while recording.
#[track_caller]
#[inline]
pub fn note(code: &'static str, detail: impl FnOnce() -> String) {
    note_at(code, Location::caller(), detail);
}

/// `note` of a decision made at `site` in a lower layer (gpui reports its own line).
#[inline]
pub fn note_at(
    code: &'static str,
    site: &'static Location<'static>,
    detail: impl FnOnce() -> String,
) {
    if enabled() && kept_here() {
        let cause = Cause {
            code,
            detail: detail(),
            site,
        };
        with(|j| match j.causes.len() < CAUSES {
            true => j.causes.push(cause),
            false => j.dropped += 1,
        });
    }
}

/// Count a hit of `cache`.
#[inline]
pub fn cache_hit(cache: &'static str) {
    if enabled() && kept_here() {
        with(|j| j.caches.entry(cache).or_default().hits += 1);
    }
}

/// Count a miss of `cache` for `reason`, made at the caller.
#[track_caller]
#[inline]
pub fn cache_miss(cache: &'static str, reason: &'static str, detail: impl FnOnce() -> String) {
    if enabled() && kept_here() {
        let site = Location::caller();
        let detail = detail();
        with(|j| {
            let stats = j.caches.entry(cache).or_default();
            let count = stats.misses.entry(reason).or_default();
            *count += 1;
            if *count as usize <= EXAMPLES {
                stats.examples.push(Cause {
                    code: reason,
                    detail,
                    site,
                });
            }
        });
    }
}

/// Bytes the process-wide caches hold now, by cache name (memory owners
/// outside any document; kept whether or not the journal records).
pub fn caches_held() -> Vec<(&'static str, usize)> {
    crate::cache::held().collect()
}

/// Everything recorded since the last take.
pub fn take() -> Journal {
    let mut out = Journal::new();
    with(|j| std::mem::swap(j, &mut out));
    out
}

/// Serializes the tests of this crate that subscribe: the journal is process-wide.
#[cfg(test)]
pub(crate) fn test_reader() -> (std::sync::MutexGuard<'static, ()>, Subscription) {
    static ONE: Shared<()> = Shared::new(());
    let guard = ONE.lock();
    let sub = subscribe();
    take();
    (guard, sub)
}

#[cfg(test)]
mod tests;
