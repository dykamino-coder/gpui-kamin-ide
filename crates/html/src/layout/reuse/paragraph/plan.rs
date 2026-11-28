//! The record a paragraph's build makes (super): the keys of its atoms,
//! their paths, and the record attached to the native paragraph.

use super::{Counters, FlowReuse, ParaRecord, Refusal, key, key::ParaKey};
use crate::dom::{BoxId, Element, Node};
use crate::layout::options::RenderOpts;
use crate::text::inline::atom_parent::AtomCx;
use crate::text::paragraph::{KeptAtoms, KeptSlot, Paragraph};
use std::cell::Cell;
use std::rc::Rc;

/// A paragraph being built for a record: the keys of its atoms in order.
pub(crate) struct Plan {
    pub(super) flow: Rc<FlowReuse>,
    pub(super) id: u64,
    pub(super) key: ParaKey,
    pub(super) owner: BoxId,
    pub(super) at: u32,
    pub(super) start: Counters,
    pub(super) keys: Cell<Vec<u64>>,
    pub(super) refusal: Cell<Option<Refusal>>,
    pub(super) widths: Option<Widths>,
}

/// The outside width reads of the build of a paragraph inside an atom: the
/// atom's own constraint (`ParaKey::atom_widths`) is an input of the record only
/// if the build read it, as LayoutNG compares only the constraint-space fields
/// a layout used.
pub(super) struct Widths {
    read: Rc<Cell<bool>>,
    saved: crate::layout::cx::ScopedWidths,
}

impl Widths {
    /// Track the paragraph's build where its key names the atom's widths.
    pub(super) fn track(opts: &RenderOpts) -> Option<Self> {
        (opts.cx.clamp.atom_depth > 0).then(|| {
            let read = Rc::new(Cell::new(false));
            let saved = opts.cx.constraints.track_scoped(read.clone());
            Self { read, saved }
        })
    }
}

impl Plan {
    /// End the tracking of the build's width reads (`Widths`): an atom's
    /// constraint the build did not read is no input of the record.
    fn untrack(&mut self, opts: &RenderOpts) {
        if let Some(Widths { read, saved }) = self.widths.take() {
            opts.cx.constraints.untrack_scoped(saved);
            if !read.get() {
                self.key.width_free();
            }
        }
    }

    /// The paragraph has no pieces: nothing is kept (`Refusal::Empty`).
    pub(crate) fn empty(mut self, opts: &RenderOpts) {
        self.untrack(opts);
        self.flow.store.paragraphs.refuse(Refusal::Empty);
    }

    /// Atom `e` of the paragraph, built with `atom_cx`.
    pub(crate) fn note_atom(&self, e: &Element, atom_cx: &AtomCx) {
        let why = if atom_cx.parent.is_some() || atom_cx.in_inline_cb {
            Some(Refusal::AtomParent)
        } else if !key::fixed_edges(e) {
            Some(Refusal::AtomWidth)
        } else {
            (e.node_id == 0).then_some(Refusal::AtomPath)
        };
        if let Some(why) = why {
            self.refusal.set(self.refusal.get().or(Some(why)));
        }
        let mut keys = self.keys.take();
        keys.push(e.node_id);
        self.keys.set(keys);
    }

    /// The plan with the paths of its atoms below `nodes`; `None` if refused.
    pub(crate) fn seal(
        mut self,
        nodes: &[Node],
        flow_text: bool,
        opts: &RenderOpts,
    ) -> Option<Sealed> {
        self.untrack(opts);
        let records = &self.flow.store.paragraphs;
        if let Some(why) = self.refusal.get() {
            records.refuse(why);
            return None;
        }
        let keys = self.keys.take();
        let Some(atoms) = paths(nodes, keys) else {
            records.refuse(Refusal::AtomPath);
            return None;
        };
        Some(Sealed {
            plan: Some(self),
            atoms,
            flow_text,
        })
    }
}

/// A plan whose atoms are located: the native paragraph attaches it; a
/// paragraph of another route drops it (Refusal::Paragraph).
pub(crate) struct Sealed {
    plan: Option<Plan>,
    atoms: KeptAtoms,
    flow_text: bool,
}

impl Sealed {
    /// Record `para`, built under `opts`, for the next frames.
    pub(crate) fn attach(mut self, para: &mut Paragraph, opts: &RenderOpts) {
        let Some(plan) = self.plan.take() else {
            return;
        };
        let (atoms, flow_text) = (std::mem::take(&mut self.atoms), self.flow_text);
        let records = &plan.flow.store.paragraphs;
        let slot = Rc::new(KeptSlot::default());
        // An atom of automatic or percentage inline size (`fixed_inline`).
        if !para.atoms_fixed() {
            records.refuse(Refusal::AtomWidth);
            return;
        }
        if !para.keep_into(slot.clone(), atoms) {
            records.refuse(Refusal::Paragraph);
            return;
        }
        let record = ParaRecord {
            key: plan.key,
            owner: plan.owner,
            at: plan.at,
            counters: (plan.start, Counters::read(&opts.cx)),
            flow_text,
            slot,
            seen: Cell::new(plan.flow.store.epoch.get()),
        };
        records.with(|map| map.insert(plan.id, Rc::new(record)));
        records.count(|s| s.recorded += 1);
        crate::work::add!(ParagraphsKept, 1);
    }
}

/// The paths of the atoms `keys` (in document order) below `nodes`: the first
/// element of each key in preorder, not entered further.
pub(super) fn paths(nodes: &[Node], keys: Vec<u64>) -> Option<KeptAtoms> {
    let mut out = KeptAtoms::default();
    // Each level: its nodes and the number of the next one to visit.
    let mut stack: Vec<(&[Node], usize)> = vec![(nodes, 0)];
    while let (Some(&key), Some(&(level, next))) = (keys.get(out.keys.len()), stack.last()) {
        let Some(node) = level.get(next) else {
            stack.pop();
            continue;
        };
        if let Some(top) = stack.last_mut() {
            top.1 += 1;
        }
        let Node::Element(e) = node else { continue };
        if e.node_id == key {
            out.keys.push(key);
            out.path.extend(stack.iter().map(|&(_, i)| i as u32 - 1));
            out.ends.push(out.path.len() as u32);
        } else {
            stack.push((e.children.as_slice(), 0));
        }
    }
    (out.keys.len() == keys.len()).then_some(out)
}

impl Drop for Sealed {
    fn drop(&mut self) {
        if let Some(plan) = &self.plan {
            plan.flow.store.paragraphs.refuse(Refusal::Paragraph);
        }
    }
}
