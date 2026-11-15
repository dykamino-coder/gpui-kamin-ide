//! Counters of the paint-only path of restyles (`document::revision::layer_write`,
//! `document::retained::box_repaint`): how many restyled boxes went into the scene
//! layer by a value write or a record of their own paint, and how many refused it, by
//! reason. Process-wide, read by benches; they cost one relaxed add per box.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Boxes whose new paint was written into the layer as values.
pub static VALUES: AtomicU64 = AtomicU64::new(0);
/// Boxes queued to record their own paint again.
pub static RECORDS: AtomicU64 = AtomicU64::new(0);
/// Boxes whose change is not own paint (layout, other values, controls).
pub static REFUSED_NOT_PAINT: AtomicU64 = AtomicU64::new(0);
/// Boxes whose paint is not their own commands (hover, decorations, root canvas...).
pub static REFUSED_NOT_OWN: AtomicU64 = AtomicU64::new(0);
/// Restyles with no recorded layer to write into.
pub static REFUSED_NO_LAYER: AtomicU64 = AtomicU64::new(0);
/// Boxes the layer refused to record again (no run, order, transform, sprites).
pub static REFUSED_BY_LAYER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn add(counter: &AtomicU64, n: u64) {
    counter.fetch_add(n, Relaxed);
}

/// The counters in the order of the names `NAMES`.
pub fn read() -> [u64; 6] {
    [
        &VALUES,
        &RECORDS,
        &REFUSED_NOT_PAINT,
        &REFUSED_NOT_OWN,
        &REFUSED_NO_LAYER,
        &REFUSED_BY_LAYER,
    ]
    .map(|c| c.load(Relaxed))
}

/// Names of `read`'s counters.
pub const NAMES: [&str; 6] = [
    "values",
    "records",
    "refused_not_paint",
    "refused_not_own",
    "refused_no_layer",
    "refused_by_layer",
];
