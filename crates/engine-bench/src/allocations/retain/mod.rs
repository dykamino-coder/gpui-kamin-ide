//! Explicitly enabled retention snapshots (see docs/memprof.md).
//! Without ENGINE_BENCH_RETAINERS_OUT, startup and allocator hooks allocate nothing.
//! All table metadata, output path and snapshot state are created after opt-in.
//! ENGINE_BENCH_RETAINERS_SLOTS sets the table size to 2^k slots (default k=23).
//! A conservative snapshot scans live blocks and the snapshot thread's roots;
//! other threads' stacks and native allocations are outside its scope.

mod config;
mod roots;
mod table;

use kamin_heap::retain::{Block, FLAG_CYCLE, FLAG_UNHELD, Graph, Memory};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release},
    },
    time::Instant,
};

const OUT: &str = "ENGINE_BENCH_RETAINERS_OUT";
const SLOTS: &str = "ENGINE_BENCH_RETAINERS_SLOTS";

struct State {
    path: PathBuf,
    taken: AtomicBool,
    table: table::Table,
}

// Only the publication pointer exists when tracking is disabled.
static STATE: AtomicPtr<State> = AtomicPtr::new(std::ptr::null_mut());

#[inline]
fn state() -> Option<&'static State> {
    // SAFETY: initialization publishes a complete immutable State with release ordering.
    // Its table/flags use atomics, and the State is never reclaimed during the process.
    unsafe { STATE.load(Acquire).as_ref() }
}

pub(crate) fn enabled() -> bool {
    state().is_some()
}

/// Checks presence without initializing the global allocator on the disabled path.
pub(crate) fn init() -> Result<(), String> {
    if !config::present() {
        return Ok(());
    }
    let Some(path) = std::env::var_os(OUT) else {
        return Ok(());
    };
    let bits = match std::env::var(SLOTS) {
        Ok(text) => text
            .parse::<u32>()
            .ok()
            .filter(|bits| (16..=30).contains(bits))
            .ok_or_else(|| format!("{SLOTS}={text}: expected 16..30"))?,
        Err(_) => 23,
    };
    let table = table::Table::new(bits)?;
    let state = Box::new(State {
        path: path.into(),
        taken: AtomicBool::new(false),
        table,
    });
    // Startup calls init once, before document work and allocator callbacks can track.
    STATE.store(Box::into_raw(state), Release);
    Ok(())
}

/// Record an allocation; disabled tracking costs one pointer load.
#[inline]
pub(super) fn track(pointer: *const u8, bytes: usize) {
    if let Some(state) = state() {
        state.table.insert(pointer as usize, bytes);
    }
}
/// Record a free before returning `pointer` to the allocator.
#[inline]
pub(super) fn untrack(pointer: *const u8) {
    if let Some(state) = state() {
        state.table.remove(pointer as usize);
    }
}

/// Live process heap memory.
struct Process;

impl Memory for Process {
    fn words(&self, address: usize, bytes: usize) -> &[usize] {
        // SAFETY: the address and size describe a live table block or root region.
        // Allocator blocks are aligned to at least a machine word.
        unsafe { std::slice::from_raw_parts(address as *const usize, bytes / size_of::<usize>()) }
    }
}

fn ms(since: Instant) -> f64 {
    (since.elapsed().as_secs_f64() * 1e4).round() / 10.0
}

/// Take one graph snapshot per process and return its benchmark JSON summary.
pub(crate) fn snapshot(mode: &str) -> Option<Value> {
    let state = state()?;
    let path = &state.path;
    let table = &state.table;
    if state.taken.swap(true, Relaxed) {
        return None;
    }
    if table.overflowed() {
        return Some(json!({"error": format!(
            "the live block table overflowed; raise {SLOTS} (now {} slots)", table.capacity())}));
    }
    let started = Instant::now();
    let mut count = 0usize;
    table.each(|_, _| count += 1);
    // The block list itself contains every address; exclude it from the graph.
    let mut blocks: Vec<Block> = Vec::with_capacity(count + count / 8 + 4096);
    let own = blocks.as_ptr() as usize;
    let mut missed = 0usize;
    table.each(|address, bytes| {
        if address == own {
            return;
        }
        if blocks.len() < blocks.capacity() {
            blocks.push(Block { address, bytes });
        } else {
            missed += 1;
        }
    });
    let collect = ms(started);
    let marker = 0usize;
    let regions = roots::regions(std::hint::black_box(&marker) as *const usize as usize);
    let scanned = Instant::now();
    let mut graph = Graph::scan(blocks, regions, &Process);
    let scan = ms(scanned);
    let analyzed = Instant::now();
    graph.analyze();
    let analyze = ms(analyzed);
    let mut summary = summary(&graph, mode);
    summary["missed_blocks"] = json!(missed);
    summary["table_slots"] = json!(table.capacity());
    summary["collect_ms"] = json!(collect);
    summary["scan_ms"] = json!(scan);
    summary["analyze_ms"] = json!(analyze);
    let written = Instant::now();
    let result = std::fs::File::create(path).and_then(|file| {
        let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
        graph.write(&mut out, &summary.to_string())?;
        std::io::Write::flush(&mut out)
    });
    summary["write_ms"] = json!(ms(written));
    summary["file"] = json!(path.display().to_string());
    if let Err(error) = result {
        summary["error"] = json!(format!("{}: {error}", path.display()));
    }
    Some(summary)
}

fn summary(graph: &Graph, mode: &str) -> Value {
    let regions: Vec<Value> = graph
        .regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            json!({"name": region.name, "bytes": region.bytes,
                "retained_bytes": graph.retained[graph.region(index) as usize]})
        })
        .collect();
    let count = |flag| graph.flags.iter().filter(|value| **value == flag).count();
    json!({
        "mode": mode,
        "blocks": graph.blocks.len(),
        "bytes": graph.retained[graph.root() as usize],
        "edges": graph.edge_to.len(),
        "regions": regions,
        "unrooted_retained_bytes": graph.retained[graph.unrooted() as usize],
        "unheld_entries": count(FLAG_UNHELD),
        "cycle_entries": count(FLAG_CYCLE),
        "scope": "live Rust heap blocks, conservative scan of every aligned word; roots: the snapshot thread's stack, the image's writable sections and its TLS",
    })
}

#[cfg(all(test, feature = "allocation-audit"))]
mod tests;
