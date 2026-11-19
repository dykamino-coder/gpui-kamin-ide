//! Counters of the paint-only path of restyles (`document::revision::layer_write`,
//! `document::retained::box_repaint`): how many restyled boxes went into the scene
//! layer by a value write or a record of their own paint, and how many refused it, by
//! reason. Process-wide, read by benches; they cost one relaxed add per box.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Boxes whose new paint was written into the layer as values.
pub static VALUES: AtomicU64 = AtomicU64::new(0);
/// Boxes queued to record their own paint again.
pub static RECORDS: AtomicU64 = AtomicU64::new(0);
/// Boxes under a block the frame skipped off the screen: neither written nor
/// recorded, their block's record takes the change.
pub static UNSEEN: AtomicU64 = AtomicU64::new(0);
/// Boxes in the layer whose runs showed no pixel of its frame: their records
/// wait for a frame that shows them (`layout::reuse::deferred`).
pub static DEFERRED: AtomicU64 = AtomicU64::new(0);
/// Boxes whose change is not own paint (layout, other values, controls).
pub static REFUSED_NOT_PAINT: AtomicU64 = AtomicU64::new(0);
/// Boxes whose paint is not their own commands (hover, decorations, root canvas...).
pub static REFUSED_NOT_OWN: AtomicU64 = AtomicU64::new(0);
/// Restyles with no recorded layer to write into.
pub static REFUSED_NO_LAYER: AtomicU64 = AtomicU64::new(0);
/// Boxes the layer refused to record again (no run, order, transform, sprites).
pub static REFUSED_BY_LAYER: AtomicU64 = AtomicU64::new(0);

/// Batches with no plan (`document::revision::plan`): the document is
/// rebuilt. Each takes one reason below.
pub static REFUSED_REBUILT: AtomicU64 = AtomicU64::new(0);
/// The revision is not quiet (diagnostics, failures, limits), the theme or
/// the source changed.
pub static REFUSED_PLAN_STATE: AtomicU64 = AtomicU64::new(0);
/// An edited element the box tree does not hold, or an attribute the frame
/// build reads (not `class` or `data-*` the sheets read).
pub static REFUSED_PLAN_ELEMENT: AtomicU64 = AtomicU64::new(0);
/// A text edit the boxes cannot take in place, or one beside a restyle.
pub static REFUSED_PLAN_TEXT: AtomicU64 = AtomicU64::new(0);
/// The restyle probe, its targets or their cascades did not resolve.
pub static REFUSED_RESTYLE_PROBE: AtomicU64 = AtomicU64::new(0);
/// `attr()` reads attributes outside selectors.
pub static REFUSED_RESTYLE_ATTR: AtomicU64 = AtomicU64::new(0);
/// A target's values outside its style struct changed (pseudo-elements, counters...).
pub static REFUSED_RESTYLE_REST: AtomicU64 = AtomicU64::new(0);
/// A target's change reaches layout.
pub static REFUSED_RESTYLE_LAYOUT: AtomicU64 = AtomicU64::new(0);
/// A restyle of `html`, of the canvas background of `body` or of an
/// unwrapped wrapper.
pub static REFUSED_RESTYLE_ROOT: AtomicU64 = AtomicU64::new(0);
/// The boxes of the batch were not found along paths, or a patch failed.
pub static REFUSED_PLAN_PATHS: AtomicU64 = AtomicU64::new(0);

/// Revisions committed by plan (`document::revision::commit`): a rebuild
/// of the document is none of them.
pub static PLAN_CLEAN: AtomicU64 = AtomicU64::new(0);
pub static PLAN_PATCHED: AtomicU64 = AtomicU64::new(0);
pub static PLAN_RESTYLED: AtomicU64 = AtomicU64::new(0);
/// Nanoseconds spent deciding and writing the paint-only path.
pub static WRITE_NS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn note_plan(counter: &AtomicU64) {
    add(counter, 1);
}

/// Adds its lifetime to `WRITE_NS`.
pub(crate) struct Timer(std::time::Instant);

impl Timer {
    pub(crate) fn start() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        add(&WRITE_NS, self.0.elapsed().as_nanos() as u64);
    }
}

pub(crate) fn add(counter: &AtomicU64, n: u64) {
    counter.fetch_add(n, Relaxed);
}

/// `value`, counting `reason` when it is `None`.
pub(crate) fn why<T>(value: Option<T>, reason: &AtomicU64) -> Option<T> {
    if value.is_none() {
        add(reason, 1);
    }
    value
}

/// `None`, counting `reason`.
pub(crate) fn refuse<T>(reason: &AtomicU64) -> Option<T> {
    add(reason, 1);
    None
}

pub mod not_own;

/// Counters before the reasons of `REFUSED_NOT_OWN` (`not_own`) in `NAMES`.
const PATH: usize = 22;

/// The counters in the order of the names `NAMES`.
pub fn read() -> [u64; NAMES.len()] {
    let path = [
        &VALUES,
        &RECORDS,
        &UNSEEN,
        &DEFERRED,
        &REFUSED_NOT_PAINT,
        &REFUSED_NOT_OWN,
        &REFUSED_NO_LAYER,
        &REFUSED_BY_LAYER,
        &REFUSED_REBUILT,
        &REFUSED_PLAN_STATE,
        &REFUSED_PLAN_ELEMENT,
        &REFUSED_PLAN_TEXT,
        &REFUSED_RESTYLE_PROBE,
        &REFUSED_RESTYLE_ATTR,
        &REFUSED_RESTYLE_REST,
        &REFUSED_RESTYLE_LAYOUT,
        &REFUSED_RESTYLE_ROOT,
        &REFUSED_PLAN_PATHS,
        &PLAN_CLEAN,
        &PLAN_PATCHED,
        &PLAN_RESTYLED,
        &WRITE_NS,
    ]
    .map(|c| c.load(Relaxed));
    let reasons = not_own::read();
    std::array::from_fn(|i| if i < PATH { path[i] } else { reasons[i - PATH] })
}

/// Names of `read`'s counters: the path's, then `not_own_<reason>` for each
/// of `not_own::REASONS`.
pub const NAMES: [&str; PATH + not_own::REASONS.len()] = [
    "values",
    "records",
    "unseen",
    "deferred",
    "refused_not_paint",
    "refused_not_own",
    "refused_no_layer",
    "refused_by_layer",
    "refused_rebuilt",
    "refused_plan_state",
    "refused_plan_element",
    "refused_plan_text",
    "refused_restyle_probe",
    "refused_restyle_attr",
    "refused_restyle_rest",
    "refused_restyle_layout",
    "refused_restyle_root",
    "refused_plan_paths",
    "plan_clean",
    "plan_patched",
    "plan_restyled",
    "write_ns",
    "not_own_root_canvas",
    "not_own_hover",
    "not_own_inline",
    "not_own_hidden",
    "not_own_clone",
    "not_own_filter",
    "not_own_transform",
    "not_own_decoration",
    "not_own_band_effect",
    "not_own_band_paint",
    "not_own_kind",
    "not_own_other",
];

/// Census of a document's boxes for the paint-only path: how many paint only
/// their own commands (`own`), and how many would refuse it, by reason
/// (`paint::box_tree::styled::own_box_refusal`, controls).
pub fn census(doc: &crate::Document) -> std::collections::BTreeMap<&'static str, u64> {
    let mut out = std::collections::BTreeMap::new();
    let state = doc.state_rc();
    let mut stack: Vec<std::slice::Iter<'_, crate::dom::Node>> = vec![doc.nodes().iter()];
    while let Some(list) = stack.last_mut() {
        let Some(node) = list.next() else {
            stack.pop();
            continue;
        };
        let crate::dom::Node::Element(e) = node else {
            continue;
        };
        let reason = if crate::document::paint_controls().contains(&e.tag.as_str()) {
            Some("control")
        } else {
            crate::paint::box_tree::styled::own_box_refusal(&state, e)
        };
        *out.entry(reason.unwrap_or("own")).or_insert(0) += 1;
        stack.push(e.children.iter());
    }
    out
}
