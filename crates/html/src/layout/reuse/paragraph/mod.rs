//! Paragraphs kept across frames (`text::paragraph::kept`).
//!
//! A new window width changes the available width of a paragraph and none of
//! what its build read: the text, runs and spans of its nodes and the boxes
//! of its atoms of fixed inline size. So the paragraph element of the last
//! frame is taken whole and only its lines are broken again, as Blink keeps
//! the inline items of `InlineNode` while `NeedsCollectInlines` is clear and
//! takes an atomic inline's `CachedLayoutResult`. The element of an atom is
//! built only where it shows (`LazyBuild`).
//!
//! A record holds while the build epoch of the container's box is not newer
//! than the record: an edit of any node below it, from the parser or from a
//! script, publishes a newer one (`BoxTree::build_epoch`), so the paragraph
//! is built and recorded again. The input from outside the nodes is compared
//! as a whole (`ParaKey`).

mod key;
mod plan;
#[cfg(test)]
mod tests;

use super::{Counters, FlowReuse};
use crate::dom::{BoxId, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::paragraph::{AtomFn, AtomSource, KeptAtoms, KeptSlot, LazyBuild, Paragraph};
use key::ParaKey;
pub(crate) use plan::{Plan, Sealed};
use std::cell::Cell;

/// Builds the element of an atom as the paragraph's build made it, under
/// the style and options of this frame (`render::paragraph::route::finish`);
/// the flag: the paragraph has text in flow.
pub(crate) type AtomBuilder = fn(&BoxStyle, &RenderOpts, bool) -> AtomFn;
use std::collections::HashMap;
use std::rc::Rc;

/// Why a paragraph was built without being kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The build context or the paragraph's route is not plain (`key::plain`).
    Context,
    /// A top-level element's subtree is not inert.
    Subtree,
    /// An atom's box depends on the available width (a percentage or an
    /// automatic inline size, percentage edges).
    AtomWidth,
    /// An atom inherits from an inline ancestor or has an inline containing block.
    AtomParent,
    /// An atom's node has no key or no path below the paragraph's nodes.
    AtomPath,
    /// The paragraph holds what a later frame could not lay out without a build.
    Paragraph,
    /// The paragraph has no atom: a text-only build is cheap, while keeping
    /// every such paragraph of a long page costs its whole model in memory.
    TextOnly,
}

/// Kept paragraphs of a document: recorded, resumed and refused by reason.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct KeepStats {
    pub(crate) recorded: u64,
    pub(crate) resumed: u64,
    pub(crate) refused: [u64; 7],
}

/// The records of a document's kept paragraphs, by `key::id`.
#[derive(Default)]
pub(crate) struct Records {
    map: Cell<HashMap<u64, Rc<ParaRecord>>>,
    /// `Store::epoch` of the last pruning.
    pruned: Cell<u64>,
    stats: Cell<KeepStats>,
}

impl Records {
    #[cfg(test)]
    pub(crate) fn stats(&self) -> KeepStats {
        self.stats.get()
    }

    /// No record holds (`Store::reset_replays`).
    pub(super) fn reset(&self) {
        self.map.take();
    }

    fn with<R>(&self, f: impl FnOnce(&mut HashMap<u64, Rc<ParaRecord>>) -> R) -> R {
        let mut map = self.map.take();
        let out = f(&mut map);
        self.map.set(map);
        out
    }

    fn count(&self, f: impl FnOnce(&mut KeepStats)) {
        let mut stats = self.stats.get();
        f(&mut stats);
        self.stats.set(stats);
    }

    fn refuse(&self, why: Refusal) {
        self.count(|s| s.refused[why as usize] += 1);
    }

    /// Drop the records no build took or made in the last two frames.
    fn prune(&self, epoch: u64) {
        if self.pruned.replace(epoch) != epoch {
            let live = |r: &Rc<ParaRecord>| r.seen.get().saturating_add(2) >= epoch;
            self.with(|map| map.retain(|_, r| live(r)));
        }
    }
}

struct ParaRecord {
    key: ParaKey,
    owner: BoxId,
    /// `BoxTree::clock` of the build.
    at: u32,
    counters: (Counters, Counters),
    flow_text: bool,
    slot: Rc<KeptSlot>,
    /// `Store::epoch` of the last build that took or made it.
    seen: Cell<u64>,
}

/// The paragraph of the last frame for this build (`Ok`), or the plan of
/// this build's record (`Err`): `plain` — no native request, line clamp or
/// first-line cut of its own.
pub(crate) fn start(
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
    plain: bool,
    builder: AtomBuilder,
) -> Result<Paragraph, Option<Box<Plan>>> {
    let Some(flow) = opts.cx.contracts.reuse.clone() else {
        return Err(None);
    };
    let records = &flow.store.paragraphs;
    let epoch = flow.store.epoch.get();
    records.prune(epoch);
    let owner = {
        let boxes = opts.cx.doc().boxes.borrow();
        boxes
            .box_of(inherited.self_node)
            .map(|id| (id, boxes.clock()))
    };
    let (Some((owner, at)), true) = (owner, plain && key::plain(inherited, opts)) else {
        records.refuse(Refusal::Context);
        return Err(None);
    };
    if opts.cx.contracts.document_root != Some(flow.doc) || nodes.is_empty() {
        return Err(None);
    }
    let id = key::id(inherited.self_node, nodes);
    let start = Counters::read(&opts.cx);
    if let Some(para) = resume(&flow, id, (nodes, inherited, opts), start, builder) {
        records.count(|s| s.resumed += 1);
        crate::work::add!(ParagraphsResumed, 1);
        return Ok(para);
    }
    let Some(reads_viewport) = key::subtree(nodes, opts) else {
        records.refuse(Refusal::Subtree);
        return Err(None);
    };
    // Equal containers share one style of the key (`KeyInputs`).
    let key = ParaKey::of(&flow.store.inputs, inherited, opts, reads_viewport);
    Err(Some(Box::new(Plan {
        flow,
        id,
        key,
        owner,
        at,
        start,
        keys: Cell::new(Vec::new()),
        refusal: Cell::new(None),
    })))
}

/// The kept paragraph of record `id`, if it holds (see the module).
fn resume(
    flow: &Rc<FlowReuse>,
    id: u64,
    (nodes, inherited, opts): (&[Node], &BoxStyle, &RenderOpts),
    start: Counters,
    builder: AtomBuilder,
) -> Option<Paragraph> {
    if !flow.allow || flow.again {
        return None;
    }
    let record = flow.store.paragraphs.with(|map| map.get(&id).cloned())?;
    let clean = {
        let boxes = opts.cx.doc().boxes.borrow();
        boxes.build_epoch(record.owner)? <= record.at
            && record.at <= boxes.clock()
            && !boxes.resized_by_siblings(record.owner, record.at)
    };
    if !clean || !record.key.holds(inherited, opts) || !record.slot.is_filled() {
        return None;
    }
    let para = record.slot.take()?;
    let source = match para.kept_atoms() {
        Some(atoms) => AtomSource::of(nodes, &atoms)?,
        None => AtomSource::of(nodes, &KeptAtoms::default())?,
    };
    let (from, to) = record.counters;
    start.advanced(from, to).write(&opts.cx);
    record.seen.set(flow.store.epoch.get());
    let lazy = LazyBuild::new(source, builder(inherited, opts, record.flow_text));
    let para = para
        .resume(record.slot.clone(), lazy)
        .measure_context(&opts.cx);
    Some(para.rebind_selection(opts.selection_color()))
}
