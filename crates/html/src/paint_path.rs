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
/// SVG content elements (no CSS box) whose change was only in the box paint
/// groups they do not paint (background, border colours): nothing to paint.
pub static SVG_UNPAINTED: AtomicU64 = AtomicU64::new(0);
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

/// Updates whose markup differs in shape (`document::revision::shape`:
/// children, attributes SVG reads as they are, texts in SVG, templates or
/// MathML): the markup is parsed and built anew.
pub static REFUSED_SHAPE: AtomicU64 = AtomicU64::new(0);
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
/// Edits wholly off the screen of boxes the layer cannot record again, kept
/// with the frame until a frame shows them (`layout::reuse::Wait`): inline
/// boxes and boxes with decoration elements.
pub static DEFERRED_INLINE: AtomicU64 = AtomicU64::new(0);
pub static DEFERRED_DECORATION: AtomicU64 = AtomicU64::new(0);
/// Such edits a replay brought onto the screen: the frame was built.
pub static ARRIVED_INLINE: AtomicU64 = AtomicU64::new(0);
pub static ARRIVED_DECORATION: AtomicU64 = AtomicU64::new(0);
/// Nanoseconds spent deciding and writing the paint-only path.
pub static WRITE_NS: AtomicU64 = AtomicU64::new(0);

#[track_caller]
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

/// Count `n` on `counter`; a refusal also goes into the DevTools `why`
/// journal as `paint_path.<name>` at the caller.
#[track_caller]
pub(crate) fn add(counter: &AtomicU64, n: u64) {
    counter.fetch_add(n, Relaxed);
    if crate::why::enabled()
        && let Some(code) = codes::refusal(counter)
    {
        crate::why::note(code, String::new);
    }
}

mod codes;
pub(crate) use codes::refuse_with;

/// `value`, counting `reason` when it is `None`.
#[track_caller]
pub(crate) fn why<T>(value: Option<T>, reason: &AtomicU64) -> Option<T> {
    if value.is_none() {
        add(reason, 1);
    }
    value
}

/// `None`, counting `reason`.
#[track_caller]
pub(crate) fn refuse<T>(reason: &AtomicU64) -> Option<T> {
    add(reason, 1);
    None
}

pub mod not_own;

/// Counters before the reasons of `REFUSED_NOT_OWN` (`not_own`) in `NAMES`.
const PATH: usize = 28;

/// The counters in the order of the names `NAMES`.
pub fn read() -> [u64; NAMES.len()] {
    let path = counters().map(|c| c.load(Relaxed));
    let reasons = not_own::read();
    std::array::from_fn(|i| if i < PATH { path[i] } else { reasons[i - PATH] })
}

/// The path's counters in the order of `NAMES`.
fn counters() -> [&'static AtomicU64; PATH] {
    [
        &VALUES,
        &RECORDS,
        &UNSEEN,
        &SVG_UNPAINTED,
        &DEFERRED,
        &REFUSED_NOT_PAINT,
        &REFUSED_NOT_OWN,
        &REFUSED_NO_LAYER,
        &REFUSED_BY_LAYER,
        &REFUSED_SHAPE,
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
        &DEFERRED_INLINE,
        &DEFERRED_DECORATION,
        &ARRIVED_INLINE,
        &ARRIVED_DECORATION,
        &WRITE_NS,
    ]
}

/// Names of `read`'s counters: the path's, then `not_own_<reason>` for each
/// of `not_own::REASONS`.
pub const NAMES: [&str; PATH + not_own::REASONS.len()] = [
    "values",
    "records",
    "unseen",
    "svg_unpainted",
    "deferred",
    "refused_not_paint",
    "refused_not_own",
    "refused_no_layer",
    "refused_by_layer",
    "refused_shape",
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
    "deferred_inline",
    "deferred_decoration",
    "arrived_inline",
    "arrived_decoration",
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
    "not_own_band_nested",
    "not_own_band_geometry",
    "not_own_kind",
    "not_own_other",
];

/// Census of a document's boxes for the paint-only path: how many paint only
/// their own commands (`own`), and how many would refuse it, by reason
/// (`paint::box_tree::styled::requested_painter`, controls).
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
            use crate::paint::box_tree::styled::{Own, requested_painter};
            match requested_painter(&state, e) {
                Own::Plain => None,
                Own::Band => Some("inline"),
                Own::Not(reason) => Some(reason.label()),
            }
        };
        *out.entry(reason.unwrap_or("own")).or_insert(0) += 1;
        stack.push(e.children.iter());
    }
    out
}
