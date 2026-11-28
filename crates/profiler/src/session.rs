//! Allocation-session lifecycle and bounded snapshots. No buffers are installed
//! until start; stop releases them after in-flight hooks leave the ledger lock.

use crate::allocation::{ACTIVE, LEDGER, Probe, Suppress};
use kamin_heap::live::{Block, Context, Event, Ledger, Totals};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

mod owners;
pub use owners::{Owner, Ownership};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub max_blocks: usize,
    pub max_events: usize,
    /// Zero selects exact observed allocations, otherwise mean byte interval.
    pub sampling_mean: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_blocks: 16_384,
            max_events: 32_768,
            sampling_mean: 16_384,
        }
    }
}

/// One allocation session per process. Process-only journal readers do not need
/// an allocation session or an allocator wrapper.
pub struct Session {
    id: u64,
    epoch: u64,
    origin: Instant,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub session: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub elapsed_ns: u64,
    pub context: Context,
    pub snapshot_ns: u64,
    pub history: Vec<Event>,
    pub totals: Totals,
    pub profiler_storage_bytes: usize,
    pub sampling_mean: usize,
    /// Observed allocations since start; pre-session allocations are excluded.
    pub live: Vec<Block>,
    /// Historical allocation data only. Never carries late owner labels.
    #[serde(skip)]
    pub peak: Option<Vec<Block>>,
    #[serde(skip)]
    pub estimated_peak: Option<kamin_heap::fold::peak::Peak>,
    pub owners: Option<Ownership>,
}

impl Session {
    pub fn start(config: Config) -> Result<Self, &'static str> {
        if config.max_blocks == 0 || config.max_blocks > 262_144 || config.max_events > 524_288 {
            return Err("profiler capacity out of bounds");
        }
        let _entered = Suppress::enter().ok_or("recursive profiler session start")?;
        let mut ledger = LEDGER.lock().unwrap_or_else(|error| error.into_inner());
        if ledger.is_some() {
            return Err("an allocation profiling session is already active");
        }
        let id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        *ledger = Some(Probe {
            ledger: Ledger::new(config.max_blocks, config.max_events),
            session: id,
            mean: config.sampling_mean,
            recording: true,
        });
        ACTIVE.store(true, Ordering::Release);
        Ok(Self {
            id,
            epoch: 0,
            origin: Instant::now(),
        })
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let _entered = Suppress::enter();
        let started = Instant::now();
        self.epoch += 1;
        let guard = LEDGER.lock().unwrap_or_else(|error| error.into_inner());
        let probe = guard
            .as_ref()
            .expect("live Session owns the installed ledger");
        let ledger = &probe.ledger;
        let live: Vec<_> = ledger.blocks().copied().collect();
        let mut snapshot = Snapshot {
            session: self.id,
            epoch: self.epoch,
            sequence: ledger.sequence(),
            elapsed_ns: self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            context: ledger.context(),
            snapshot_ns: 0,
            history: ledger.history().copied().collect(),
            totals: ledger.totals(),
            profiler_storage_bytes: ledger.storage_bytes(),
            sampling_mean: probe.mean,
            live,
            peak: None,
            estimated_peak: None,
            owners: None,
        };
        // Neither sorting nor replay allocates while another allocator hook
        // waits for the ledger. The copied prefix is a coherent historical cut.
        drop(guard);
        snapshot.live.sort_by_key(|block| block.address);
        snapshot.peak = kamin_heap::live::observed_peak(&snapshot.history, snapshot.totals);
        snapshot.estimated_peak = kamin_heap::fold::peak::estimated(
            &snapshot.history,
            snapshot.totals,
            snapshot.sampling_mean,
        );
        snapshot.snapshot_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        snapshot
    }

    /// Call on a coherent engine boundary, with the document owned by that
    /// thread. Never hold the allocation lock during an owner walk: owners may
    /// lock a cache whose worker is allocating. Claims match only unchanged IDs
    /// present both before and after the walk, including interior pointers.
    pub fn snapshot_with_owners(
        &mut self,
        walk: impl FnOnce(&mut kamin_heap::Ops<'_>),
    ) -> Snapshot {
        let _entered = Suppress::enter();
        let before = self.snapshot();
        let claims = owners::walk(&before.live, walk);
        let mut after = self.snapshot();
        after.owners = Some(owners::match_claims(
            before.epoch,
            &before.live,
            &after.live,
            claims,
        ));
        after
    }

    /// Mark a coherent engine phase without a stack walk or symbol lookup.
    pub fn mark(&mut self, frame: u64, phase: u32) {
        let _entered = Suppress::enter();
        let mut ledger = LEDGER.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(probe) = ledger.as_mut() {
            probe.ledger.set_context(Context { frame, phase });
        }
    }

    /// Select allocations at phase boundaries. Frees always retire previously
    /// observed IDs, including while allocation selection is paused.
    pub fn recording(&mut self, enabled: bool) {
        let _entered = Suppress::enter();
        if let Some(probe) = LEDGER
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            probe.recording = enabled;
        }
    }

    pub fn stop(self) -> Snapshot {
        self.finish()
    }

    fn finish(mut self) -> Snapshot {
        self.snapshot()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _entered = Suppress::enter();
        ACTIVE.store(false, Ordering::Release);
        let old = LEDGER
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        // Destroy metadata outside the lock. The disabled adapter forwards it.
        drop(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_exclusivity_bounds_and_stop_release_all_metadata() {
        assert!(
            Session::start(Config {
                max_blocks: 0,
                max_events: 0,
                sampling_mean: 0,
            })
            .is_err()
        );
        let mut session = Session::start(Config {
            max_blocks: 4,
            max_events: 10,
            sampling_mean: 0,
        })
        .unwrap();
        assert!(Session::start(Config::default()).is_err());
        crate::allocation::with_hook(|ledger| {
            let ledger = ledger.unwrap();
            ledger.allocate(16, 20);
        });
        let snapshot = session.snapshot();
        assert_eq!(snapshot.totals.live_bytes, 20);
        assert!(snapshot.profiler_storage_bytes > 0);
        drop(session);
        assert!(!ACTIVE.load(Ordering::Relaxed));
        assert!(LEDGER.lock().unwrap().is_none());
        assert!(
            Session::start(Config {
                max_blocks: 1,
                max_events: 0,
                sampling_mean: 0,
            })
            .is_ok()
        );
    }
}
