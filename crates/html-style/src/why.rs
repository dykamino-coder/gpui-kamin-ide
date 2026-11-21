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

use std::collections::BTreeMap;
use std::panic::Location;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// Version of the journal's shape and codes for protocol readers.
pub const SCHEMA: u32 = 1;
/// Causes kept between two takes; later ones are only counted.
const CAUSES: usize = 512;
/// Examples kept per cache miss reason between two takes.
const EXAMPLES: usize = 4;

static ON: AtomicBool = AtomicBool::new(false);
static JOURNAL: Mutex<Journal> = Mutex::new(Journal::new());

/// Whether the journal records.
#[inline]
pub fn enabled() -> bool {
    ON.load(Relaxed)
}

/// Start or stop recording; stopping drops what was not taken.
pub fn set_enabled(on: bool) {
    ON.store(on, Relaxed);
    if !on {
        take();
    }
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
        format!("{}:{}", self.site.file().replace('\\', "/"), self.site.line())
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
}

impl Journal {
    const fn new() -> Self {
        Self {
            causes: Vec::new(),
            dropped: 0,
            caches: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.causes.is_empty() && self.dropped == 0 && self.caches.is_empty()
    }
}

fn with(f: impl FnOnce(&mut Journal)) {
    f(&mut JOURNAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
}

/// Record a decision `code` made at the caller; `detail` runs only while recording.
#[track_caller]
#[inline]
pub fn note(code: &'static str, detail: impl FnOnce() -> String) {
    note_at(code, Location::caller(), detail);
}

/// `note` of a decision made at `site` in a lower layer (gpui reports its own line).
#[inline]
pub fn note_at(code: &'static str, site: &'static Location<'static>, detail: impl FnOnce() -> String) {
    if enabled() {
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
    if enabled() {
        with(|j| j.caches.entry(cache).or_default().hits += 1);
    }
}

/// Count a miss of `cache` for `reason`, made at the caller.
#[track_caller]
#[inline]
pub fn cache_miss(cache: &'static str, reason: &'static str, detail: impl FnOnce() -> String) {
    if enabled() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The journal is process-wide: one test owns it.
    #[test]
    fn records_only_while_enabled_with_the_callers_line() {
        set_enabled(false);
        note("off", || unreachable!("detail of a disabled journal"));
        cache_miss("c", "new_key", || unreachable!());
        assert!(take().is_empty());

        set_enabled(true);
        let line = line!() + 1;
        note("code", || "detail".into());
        cache_hit("c");
        for _ in 0..EXAMPLES + 2 {
            cache_miss("c", "evicted", String::new);
        }
        let j = take();
        set_enabled(false);
        // Other tests may record while this one has the journal on.
        let mine: Vec<_> = j.causes.iter().filter(|c| c.code == "code").collect();
        assert_eq!(mine.len(), 1);
        assert!(mine[0].site().ends_with(&format!("why.rs:{line}")));
        let c = &j.caches["c"];
        assert_eq!((c.hits, c.misses["evicted"]), (1, EXAMPLES as u64 + 2));
        assert_eq!(c.examples.len(), EXAMPLES);

        set_enabled(true);
        for _ in 0..CAUSES + 3 {
            note("many", String::new);
        }
        let j = take();
        set_enabled(false);
        assert_eq!(j.causes.len(), CAUSES);
        assert!(j.dropped >= 3);
    }
}
