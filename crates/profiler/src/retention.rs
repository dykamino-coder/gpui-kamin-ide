//! Bounded conservative retention capture. Memory is copied through the OS,
//! never exposed as Rust references into concurrently mutable application data.
//! A capture is historical and can be incomplete; unrooted does not prove a leak.

use crate::{
    allocation::{LEDGER, Suppress},
    session::Snapshot,
};
use kamin_heap::retain::{Graph, Memory, Region};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

mod read;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub blocks: usize,
    pub copied_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            blocks: 16_384,
            copied_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Coverage {
    pub attempted_bytes: usize,
    pub copied_bytes: usize,
    pub unreadable_regions: u64,
    pub changed_blocks: u64,
    pub truncated_regions: u64,
    pub capture_ns: u64,
}

pub struct Capture {
    pub session: u64,
    pub epoch: u64,
    pub graph: Graph,
    pub allocation_ids: Vec<u64>,
    pub coverage: Coverage,
}

struct Copied(BTreeMap<usize, Vec<usize>>);

impl Memory for Copied {
    fn words(&self, address: usize, bytes: usize) -> &[usize] {
        self.0.get(&address).map_or(&[], |words| {
            &words[..words.len().min(bytes / size_of::<usize>())]
        })
    }
}

/// Call at a coherent engine boundary. Worker writes can still produce a
/// non-atomic conservative view; allocation IDs are verified under the hook's
/// lock before each block copy. No owner callback runs under that lock.
pub fn capture(
    snapshot: &Snapshot,
    regions: Vec<Region>,
    limits: Limits,
    cancelled: &AtomicBool,
) -> Result<Capture, &'static str> {
    if limits.blocks == 0
        || limits.blocks > 65_536
        || limits.copied_bytes > 64 * 1024 * 1024
        || regions.len() > 1024
    {
        return Err("retention capture limits out of bounds");
    }
    let mut end = 0;
    let mut identities = std::collections::BTreeSet::new();
    for block in snapshot.live.iter().take(limits.blocks) {
        let next = block
            .address
            .checked_add(block.bytes)
            .ok_or("retention block address overflow")?;
        if block.address == 0
            || block.bytes == 0
            || block.address < end
            || block.id == 0
            || !identities.insert(block.id)
        {
            return Err("invalid retention allocation identities or ranges");
        }
        end = next;
    }
    if regions
        .iter()
        .any(|region| region.start.checked_add(region.bytes).is_none())
    {
        return Err("retention root address overflow");
    }
    let _suppress = Suppress::enter();
    let started = Instant::now();
    let mut copied = Copied(BTreeMap::new());
    let mut coverage = Coverage::default();
    let mut blocks = Vec::new();
    let mut ids = Vec::new();
    for block in snapshot.live.iter().take(limits.blocks) {
        if cancelled.load(Ordering::Relaxed) {
            return Err("retention capture cancelled");
        }
        let remaining = limits.copied_bytes.saturating_sub(coverage.attempted_bytes);
        let length = block.bytes.min(remaining) / size_of::<usize>();
        // Allocate scratch outside the ledger lock. Only the bounded OS copy
        // excludes tracked dealloc/realloc on another thread.
        let mut words = vec![0; length];
        let ledger = LEDGER.lock().unwrap_or_else(|error| error.into_inner());
        let unchanged = ledger.as_ref().is_some_and(|probe| {
            probe.session == snapshot.session
                && probe
                    .ledger
                    .block(block.address)
                    .is_some_and(|live| live.id == block.id)
        });
        if !unchanged {
            coverage.changed_blocks += 1;
            continue;
        }
        let read = read::words(block.address, &mut words);
        drop(ledger);
        record(
            block.address,
            block.bytes,
            words,
            read,
            &mut copied,
            &mut coverage,
        );
        blocks.push(kamin_heap::retain::Block {
            address: block.address,
            bytes: block.bytes,
        });
        ids.push(block.id);
    }
    coverage.truncated_regions += snapshot.live.len().saturating_sub(limits.blocks) as u64;
    for region in &regions {
        if cancelled.load(Ordering::Relaxed) {
            return Err("retention capture cancelled");
        }
        let remaining = limits.copied_bytes.saturating_sub(coverage.attempted_bytes);
        let length = region.bytes.min(remaining) / size_of::<usize>();
        let mut words = vec![0; length];
        let read = read::words(region.start, &mut words);
        record(
            region.start,
            region.bytes,
            words,
            read,
            &mut copied,
            &mut coverage,
        );
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("retention capture cancelled");
    }
    let mut graph = Graph::scan(blocks, regions, &copied);
    drop(copied);
    if cancelled.load(Ordering::Relaxed) {
        return Err("retention capture cancelled");
    }
    graph.analyze();
    if cancelled.load(Ordering::Relaxed) {
        return Err("retention capture cancelled");
    }
    coverage.capture_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    Ok(Capture {
        session: snapshot.session,
        epoch: snapshot.epoch,
        graph,
        allocation_ids: ids,
        coverage,
    })
}

fn record(
    address: usize,
    requested_bytes: usize,
    words: Vec<usize>,
    read: bool,
    copied: &mut Copied,
    coverage: &mut Coverage,
) {
    let bytes = words.len() * size_of::<usize>();
    // Charge attempts too: unreadable ranges must not bypass the work budget.
    coverage.attempted_bytes += bytes;
    if bytes < requested_bytes / size_of::<usize>() * size_of::<usize>() {
        coverage.truncated_regions += 1;
    }
    if read {
        coverage.copied_bytes += bytes;
        // A root region can start at an allocation address. Keep the longer
        // successful copy rather than hiding words already read from it.
        match copied.0.entry(address) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(words);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get().len() < words.len() {
                    entry.insert(words);
                }
            }
        }
    } else {
        coverage.unreadable_regions += 1;
    }
}
