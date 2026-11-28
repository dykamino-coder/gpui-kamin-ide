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
mod resume;
#[cfg(test)]
mod tests;

use super::{Counters, FlowReuse};
use crate::dom::{BoxId, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::paragraph::{AtomFn, KeptSlot, Paragraph};
use key::ParaKey;
pub(crate) use plan::{Plan, Sealed};
use std::cell::Cell;

/// Builds the element of an atom as the paragraph's build made it, under
/// the style and options of this frame (`render::paragraph::route::finish`);
/// the flag: the paragraph has text in flow.
pub(crate) type AtomBuilder = fn(&BoxStyle, &RenderOpts, bool) -> AtomFn;
use std::collections::HashMap;
use std::rc::Rc;

mod refusal;
pub(crate) use refusal::Refusal;

/// Kept paragraphs of a document: recorded, resumed and refused by reason.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct KeepStats {
    pub(crate) recorded: u64,
    pub(crate) resumed: u64,
    pub(crate) refused: [u64; refusal::REFUSALS],
    /// Records that did not hold, by reason (`resume::Miss`).
    pub(crate) missed: [u64; resume::MISSES],
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
        why.note();
    }

    fn miss(&self, why: resume::Miss) {
        self.count(|s| s.missed[why as usize] += 1);
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
    // A run of an anonymous box carries a style naming no element (`self_node` 0): its
    // owner is the box whose published flow the nodes are (`BoxTree::flow_of`), for a
    // render copy of a box's children the parent box of its elements (`owner_of`), else
    // the block whose lines the run makes (`InlineContext::trim_para`), whose build epoch
    // covers every node below it. The record key names that owner and the run's nodes.
    let owner = {
        let boxes = opts.cx.doc().boxes.borrow();
        let block = || (opts.cx.inline.trim_para).and_then(|(block, _)| boxes.box_of(block));
        let owner = match inherited.self_node {
            0 => (boxes.flow_of(nodes).or_else(|| boxes.owner_of(nodes))).or_else(block),
            node => boxes.box_of(node),
        };
        owner.map(|id| (id, boxes.clock()))
    };
    let why = match (owner, key::plain(inherited, opts)) {
        _ if !plain => Err(Refusal::Context),
        (None, _) => Err(Refusal::NoBox),
        (Some(_), Err(why)) => Err(why),
        (Some(owner), Ok(())) => Ok(owner),
    };
    let (owner, at) = match why {
        Ok(owner) => owner,
        Err(why) => {
            if why == Refusal::NoBox {
                note_no_box(nodes, inherited);
            }
            records.refuse(why);
            return Err(None);
        }
    };
    if opts.cx.contracts.document_root != Some(flow.doc) || nodes.is_empty() {
        return Err(None);
    }
    let id = match inherited.self_node {
        0 => key::id(super::content::box_key(owner), nodes),
        node => key::id(node, nodes),
    };
    let start = Counters::read(&opts.cx);
    if let Some(para) = resume::resume(&flow, id, (nodes, inherited, opts), start, builder) {
        records.count(|s| s.resumed += 1);
        crate::work::add!(ParagraphsResumed, 1);
        return Ok(para);
    }
    let reads_viewport = match key::subtree(nodes, opts) {
        Ok(reads) => reads,
        Err(why) => {
            records.refuse(why);
            return Err(None);
        }
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
        widths: plan::Widths::track(opts),
    })))
}

/// The `why` journal of a paragraph whose container has no box: which
/// container and what its first nodes are. Built only for a reader.
fn note_no_box(nodes: &[Node], inherited: &BoxStyle) {
    crate::why::note("paragraph.no_box", || {
        let first: Vec<String> = (nodes.iter().take(3))
            .map(|n| match n {
                Node::Element(e) => format!("<{} {:x}>", e.tag, e.node_id),
                Node::Text(t) => {
                    let blank = t.as_str().trim_ascii().is_empty();
                    format!("text {} blank {blank}", t.as_str().len())
                }
            })
            .collect();
        format!(
            "container {:x} display {:?} icb {} nodes {} {first:?}",
            inherited.self_node,
            inherited.reset_box_layout.display,
            inherited.reset_context.icb,
            nodes.len()
        )
    });
}
