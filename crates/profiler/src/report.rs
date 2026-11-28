//! One wire format for DevTools and the offline runner. Byte domains remain
//! distinct; historical peaks do not inherit owners found by a later walk.

use crate::{session::Snapshot, symbols::Symbols};
use kamin_heap::{
    fold::{self, Amount, Report},
    live::Block,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

pub const SCHEMA: u32 = 1;

/// Supplied by the launcher/build, rather than inferred from a different binary.
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Build {
    pub executable: String,
    pub revision: String,
    pub features: Vec<String>,
    pub symbols: String,
}

fn amount(value: Amount) -> Value {
    json!({
        "observed_blocks": value.observed_blocks,
        "observed_requested_bytes": value.observed_bytes,
        "estimated_blocks": value.estimated_blocks,
        "estimated_requested_bytes": value.estimated_bytes,
    })
}

fn groups(values: &BTreeMap<String, Amount>) -> Value {
    values
        .iter()
        .map(|(key, value)| (key.clone(), amount(*value)))
        .collect()
}

pub fn folded(report: &Report) -> Value {
    json!({
        "total": amount(report.total),
        "by_site": groups(&report.by_site),
        "by_subsystem": groups(&report.by_subsystem),
        "by_owned_type": groups(&report.by_owned_type),
        "unresolved_blocks": report.unresolved_blocks,
        "truncated_stacks": report.truncated_stacks,
    })
}

/// Signed changes in like-for-like byte domains. Cross-session IDs are never
/// matched, and late owner labels are not applied to either historical peak.
pub fn diff(before: &Snapshot, after: &Snapshot, a: &mut Symbols, b: &mut Symbols) -> Value {
    let _suppress = crate::allocation::Suppress::enter();
    let before = fold::fold(&before.live, before.sampling_mean, |stack| a.stack(stack));
    let after = fold::fold(&after.live, after.sampling_mean, |stack| b.stack(stack));
    fn deltas(a: &BTreeMap<String, Amount>, b: &BTreeMap<String, Amount>) -> Value {
        fold::diff(a, b)
            .into_iter()
            .map(|(key, delta)| {
                (
                    key,
                    json!({
                        "observed_blocks": delta.observed_blocks,
                        "observed_requested_bytes": delta.observed_bytes,
                        "estimated_blocks": delta.estimated_blocks,
                        "estimated_requested_bytes": delta.estimated_bytes,
                    }),
                )
            })
            .collect()
    }
    json!({
        "schema": SCHEMA,
        "before": folded(&before), "after": folded(&after),
        "delta": {
            "by_site": deltas(&before.by_site, &after.by_site),
            "by_subsystem": deltas(&before.by_subsystem, &after.by_subsystem),
            "by_owned_type": deltas(&before.by_owned_type, &after.by_owned_type),
        },
    })
}

fn blocks(blocks: &[Block]) -> Value {
    blocks.iter().map(|block| json!({
        "id": block.id, "address": format!("0x{:x}", block.address),
        "requested_bytes": block.bytes, "born": block.born,
        "frame": block.context.frame, "phase": block.context.phase,
        "stack": block.stack.addresses[..usize::from(block.stack.depth).min(block.stack.addresses.len())]
            .iter().map(|address| format!("0x{address:x}")).collect::<Vec<_>>(),
    })).collect()
}

/// Symbolization and JSON run after snapshot capture, under recursion suppression
/// when the application has installed Profiled<A>. They never run in its hook.
pub fn snapshot(snapshot: &Snapshot, build: &Build, symbols: &mut Symbols, raw: bool) -> Value {
    let _suppress = crate::allocation::Suppress::enter();
    let started = Instant::now();
    let live = fold::fold(&snapshot.live, snapshot.sampling_mean, |stack| {
        symbols.stack(stack)
    });
    let observed_peak = snapshot.peak.as_ref().map(|blocks| {
        json!({
            "sequence": snapshot.totals.peak_sequence,
            "report": folded(&fold::fold(blocks, snapshot.sampling_mean, |stack| symbols.stack(stack))),
            "owners": Value::Null,
        })
    });
    let estimated_peak = snapshot.estimated_peak.as_ref().map(|peak| {
        json!({
            "sequence": peak.sequence,
            "report": folded(&fold::fold(&peak.blocks, snapshot.sampling_mean, |stack| symbols.stack(stack))),
            "owners": Value::Null,
        })
    });
    let ownership = snapshot.owners.as_ref().map(|owners| {
        let by_id: BTreeMap<_, _> = snapshot
            .live
            .iter()
            .map(|block| (block.id, block))
            .collect();
        let mut by_owner = BTreeMap::<String, Amount>::new();
        for owner in &owners.owners {
            if let Some(block) = by_id.get(&owner.allocation) {
                by_owner
                    .entry(owner.path.to_owned())
                    .or_default()
                    .add(Amount::block(block.bytes, snapshot.sampling_mean));
            }
        }
        json!({
            "begin_epoch": owners.begin_epoch, "end_epoch": snapshot.epoch,
            "by_owner": groups(&by_owner), "reported_bytes": owners.reported_bytes,
            "unreported_observed_requested_bytes": owners.unreported_observed_bytes,
            "unmatched_claims": owners.unmatched_claims, "claims_dropped": owners.claims_dropped,
        })
    });
    let profiler_snapshot_bytes = (snapshot.live.capacity()
        + snapshot.peak.as_ref().map_or(0, Vec::capacity)
        + snapshot
            .estimated_peak
            .as_ref()
            .map_or(0, |peak| peak.blocks.capacity()))
        * size_of::<Block>()
        + snapshot.history.capacity() * size_of::<kamin_heap::live::Event>();
    json!({
        "schema": SCHEMA, "build": build, "session": snapshot.session,
        "epoch": snapshot.epoch, "sequence": snapshot.sequence,
        "elapsed_ns": snapshot.elapsed_ns, "frame": snapshot.context.frame, "phase": snapshot.context.phase,
        "coverage": {
            "scope": "allocations observed since session start",
            "sampling_mean_bytes": snapshot.sampling_mean,
            "sampling": if snapshot.sampling_mean == 0 { "exact observed allocations" } else { "Poisson byte sampling" },
            "missing_allocations": snapshot.totals.missing_allocations,
            "history_dropped": snapshot.totals.history_dropped,
            "historical_peak_available": observed_peak.is_some(),
            "complete_observed_ledger": snapshot.totals.missing_allocations == 0,
        },
        "live": folded(&live), "observed_peak": observed_peak, "estimated_peak": estimated_peak,
        "ownership": ownership,
        "profiler": {
            "ledger_storage_bytes": snapshot.profiler_storage_bytes,
            "snapshot_storage_bytes": profiler_snapshot_bytes,
            "capture_ns": snapshot.snapshot_ns,
            "report_ns": started.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            "storage_scope": "ledger and snapshot buffers only; excludes symbol/report/owner scratch",
        },
        "allocations": if raw { blocks(&snapshot.live) } else { Value::Null },
    })
}
