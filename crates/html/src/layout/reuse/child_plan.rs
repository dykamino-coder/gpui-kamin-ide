//! Prepared child lists of block containers kept across frames (`FlowReuse::child_plan`).
//!
//! The list a container's children make after the box-tree fixups, margin collapsing,
//! first-line routes and float hosts depends on the DOM and styles below the container,
//! on the container's box style and on a few scalars of its context, and on the width
//! only when the preparation read it. A frame at a new width takes the list as it was:
//! Blink keeps its layout tree across a resize and Gecko its frame tree; neither rebuilds
//! the box structure for a new width. The key is the container's box and build epoch
//! (any change of the DOM or of a style below it, a script mutation included), its box
//! values, the nodes it was given, the scalars, and the outside widths only when read.

use super::FlowReuse;
use crate::dom::{BoxId, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

/// The kept child lists of one document, by container box.
#[derive(Default)]
pub(crate) struct ChildPlans {
    map: Cell<HashMap<u64, Rc<Entry>>>,
    /// `Store::epoch` of the last pruning.
    pruned: Cell<u64>,
    stats: Cell<PlanStats>,
}

/// Prepared lists built, kept and reused (the work counters name them for the journal).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PlanStats {
    pub(crate) built: u32,
    pub(crate) kept: u32,
    pub(crate) reused: u32,
    /// Containers without a box of their own (anonymous ones): prepared every time.
    pub(crate) no_box: u32,
}

struct Entry {
    owner: BoxId,
    /// `BoxTree::build_epoch` of the owner when the list was prepared.
    at: u32,
    /// The nodes the container was given: address and length of the slice, and the nodes.
    /// Their copies hold the snapshots of their children, so an edit below them (copy on
    /// write from the root) gives them new snapshots: a shallow compare sees any edit.
    nodes: (usize, usize),
    sources: Vec<Node>,
    style: BoxStyle,
    inputs: Box<[u32]>,
    /// The width accessors the preparation read (`scoped::CB_WIDTH`, `AVAIL_W`) and their values.
    reads: u8,
    widths: (Option<f32>, Option<f32>),
    plan: Rc<dyn Any>,
    /// `Store::epoch` of the last build that took or made it.
    seen: Cell<u64>,
}

impl ChildPlans {
    /// No list holds (`Store::reset_replays`).
    pub(super) fn reset(&self) {
        self.map.take();
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> PlanStats {
        self.stats.get()
    }

    fn with<R>(&self, f: impl FnOnce(&mut HashMap<u64, Rc<Entry>>) -> R) -> R {
        let mut map = self.map.take();
        let out = f(&mut map);
        self.map.set(map);
        out
    }

    fn count(&self, f: impl FnOnce(&mut PlanStats)) {
        let mut stats = self.stats.get();
        f(&mut stats);
        self.stats.set(stats);
    }

    /// Lists no build took for two frames go: their containers left the document or
    /// are no longer built (a replayed subtree builds nothing below it).
    fn prune(&self, epoch: u64) {
        if self.pruned.replace(epoch) != epoch {
            let live = |e: &Rc<Entry>| e.seen.get().saturating_add(2) >= epoch;
            self.with(|map| map.retain(|_, e| live(e)));
        }
    }
}

impl Entry {
    fn holds(&self, key: &Key<'_>, opts: &RenderOpts) -> bool {
        self.owner == key.owner
            && self.at == key.at
            && self.nodes == key.nodes
            && *self.inputs == *key.inputs
            && same_nodes(&self.sources, key.sources)
            && self.style.same_box_values(key.style)
            && (self.reads == 0 || opts.cx.constraints.read_widths(self.reads) == self.widths)
    }
}

struct Key<'a> {
    owner: BoxId,
    at: u32,
    nodes: (usize, usize),
    sources: &'a [Node],
    style: &'a BoxStyle,
    inputs: &'a [u32],
}

impl FlowReuse {
    /// The prepared child list of the container styled `inherited` for nodes `nodes`:
    /// the kept one if its key holds, else `make`'s, kept unless it read the viewport.
    /// `inputs` are the scalars of the context the preparation reads.
    pub(crate) fn child_plan<T: 'static>(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
        inputs: &[u32],
        make: impl FnOnce() -> T,
    ) -> Rc<T> {
        let plans = &self.store.child_plans;
        let epoch = self.store.epoch.get();
        plans.prune(epoch);
        let owner = (inherited.self_node != 0)
            .then(|| {
                let boxes = opts.cx.doc().boxes.borrow();
                let id = boxes.box_of(inherited.self_node)?;
                Some((id, boxes.build_epoch(id)?))
            })
            .flatten();
        let Some((owner, at)) = owner else {
            crate::work::add!(ChildPlansNoBox, 1);
            plans.count(|s| s.no_box += 1);
            return Rc::new(make());
        };
        let id = super::content::box_key(owner);
        let key = Key {
            owner,
            at,
            nodes: (nodes.as_ptr() as usize, nodes.len()),
            sources: nodes,
            style: inherited,
            inputs,
        };
        let kept = plans.with(|map| map.get(&id).cloned());
        if let Some(entry) = kept.filter(|e| e.holds(&key, opts))
            && let Ok(plan) = entry.plan.clone().downcast::<T>()
        {
            entry.seen.set(epoch);
            crate::work::add!(ChildPlansReused, 1);
            plans.count(|s| s.reused += 1);
            return plan;
        }
        let read = Rc::new(Cell::new(0));
        let saved = opts.cx.constraints.track_reads(read.clone());
        let (plan, viewport) = crate::values::viewport_reads::track(opts.cx.vcx().doc, make);
        let reads = read.get();
        let widths = opts.cx.constraints.read_widths(reads);
        opts.cx.constraints.untrack_reads(saved);
        crate::work::add!(ChildPlansBuilt, 1);
        plans.count(|s| s.built += 1);
        let plan = Rc::new(plan);
        if viewport {
            // A viewport-relative value folded in the preparation (a float's `vw` margin).
            crate::work::add!(ChildPlansViewport, 1);
            plans.with(|map| map.remove(&id));
            return plan;
        }
        plans.count(|s| s.kept += 1);
        let entry = Entry {
            owner,
            at,
            nodes: key.nodes,
            sources: nodes.to_vec(),
            style: inherited.clone(),
            inputs: inputs.into(),
            reads,
            widths,
            plan: plan.clone(),
            seen: Cell::new(epoch),
        };
        plans.with(|map| map.insert(id, Rc::new(entry)));
        plan
    }
}

/// The same nodes apart from their children's contents: equal values (`dom::same_box`) and
/// the same children snapshots, or equal texts.
fn same_nodes(kept: &[Node], now: &[Node]) -> bool {
    kept.len() == now.len()
        && kept.iter().zip(now).all(|pair| match pair {
            (Node::Element(a), Node::Element(b)) => {
                crate::dom::same_box(a, b) && a.children.mark().holds(&b.children)
            }
            (Node::Text(a), Node::Text(b)) => {
                a.first_source() == b.first_source() && a.as_str() == b.as_str()
            }
            _ => false,
        })
}
