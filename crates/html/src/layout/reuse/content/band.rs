//! The key of a float band host's retained layout result (`band_flow::HostSlot`).
//!
//! A band host's plan (the places and sizes of its children around its
//! floats) is a function of its subtree, the containing-block width it is
//! measured at, and what its build reads from outside the subtree: the same
//! environment a content record keys on (`KeyEnv`), the host's identity, the
//! anchor counter its build starts from, the viewport and the images' layout
//! facts (`ImageReadView::layout_facts`: a decoded image changes an intrinsic
//! size without a box epoch, while the store's own revision moves with every
//! document revision). The width is the measure's own input and keys each
//! kept plan; the subtree is clean while the host's build epoch is not newer
//! than the publication of the record (`BoxTree::build_epoch`). Blink keys
//! `LayoutResult` by the `ConstraintSpace` the same way; the epoch replaces
//! its dirty bits.
use super::key::KeyEnv;
use crate::document::boxes::BoxTree;
use crate::dom::{BoxId, Node};
use crate::layout::options::RenderOpts;
use crate::layout::reuse::{Counters, FlowReuse};
use crate::style::computed::BoxStyle;
use std::rc::Rc;

/// What a band host's plan reads from outside its subtree.
pub(crate) struct BandKey {
    env: Rc<KeyEnv>,
    ids: [u64; 4],
    /// The anchor counter the build starts from; the paint key counter is
    /// not an input of a geometry and moves with every box drawn before.
    anchors: u32,
    viewport: (f32, f32),
    images: u64,
    /// The walk of a synthetic host to its boxes (`boxed_elements`); 0 for a
    /// host with a box of its own.
    shape: u64,
}

impl BandKey {
    /// The same outside input.
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.differs(other).is_none()
    }

    /// The first outside input that differs from `other`'s, for the `why` journal.
    pub(crate) fn differs(&self, other: &Self) -> Option<&'static str> {
        match () {
            _ if !(Rc::ptr_eq(&self.env, &other.env) || self.env.same(&other.env)) => {
                Some(self.env.differs(&other.env))
            }
            _ if self.ids != other.ids => Some("ids"),
            _ if self.anchors != other.anchors => Some("anchors"),
            _ if self.viewport != other.viewport => Some("viewport"),
            _ if self.images != other.images => Some("images"),
            _ if self.shape != other.shape => Some("shape"),
            _ => None,
        }
    }
}

#[cfg(test)]
impl BandKey {
    /// A key of default inputs at viewport width `width`.
    pub(crate) fn for_tests(width: f32) -> Self {
        Self {
            env: Rc::new(KeyEnv {
                inherited: Default::default(),
                avail_h: None,
                cell_bfc: false,
                outer_cell_bfc: false,
                fresh: false,
                root: Default::default(),
                viewport_height: 0.0,
                text: Default::default(),
                normal_line_height: 0.0,
                fonts: 0,
            }),
            ids: [0; 4],
            anchors: 0,
            viewport: (width, 0.0),
            images: 0,
            shape: 0,
        }
    }
}

impl FlowReuse {
    /// The boxes and key of the band host `node_id` of children `nodes`,
    /// built from `inherited` under `opts`, if this build may keep its
    /// result: a host of this document whose boxes are known (`owner`).
    pub(crate) fn band_key(
        &self,
        node_id: u64,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Option<(BandBoxes, BandKey, u64)> {
        if opts.cx.contracts.document_root != Some(self.doc) {
            crate::why::note("band_slot.none", || "another document".into());
            return None;
        }
        let frame = self.store.epoch();
        let (owner, seen, shape) = {
            let mut boxes = opts.cx.doc().boxes.try_borrow_mut().ok()?;
            let (owner, shape) = match owner(&boxes, node_id, nodes, inherited.cb_node) {
                Ok(owner) => owner,
                Err(why) => {
                    crate::why::note("band_slot.none", || {
                        format!("no box answers for the host {node_id}: {why}")
                    });
                    return None;
                }
            };
            let seen = boxes
                .slot_mut(owner.slot)
                .map(|s| *s.band_seen.get_or_insert(frame));
            (owner, seen, shape)
        };
        // The first frame of a host records nothing: a record pays off only
        // in a later frame, and a page drawn once must not pay for it.
        if seen.is_none_or(|seen| seen >= frame) {
            crate::why::note("band_slot.none", || "first frame of the host".into());
            return None;
        }
        let env = KeyEnv::of(&self.store.inputs, inherited, opts, opts, false);
        let vcx = opts.cx.vcx();
        let key = BandKey {
            env: self.store.inputs.env(&env),
            ids: super::key::ids(inherited),
            anchors: Counters::read(&opts.cx).anchors(),
            viewport: vcx.viewport,
            images: vcx.doc.images.borrow().snapshot().layout_facts(),
            shape,
        };
        Some((owner, key, frame))
    }

    /// Kept results may be read: this build is no rebuild without reuse.
    pub(crate) fn allows_kept(&self) -> bool {
        self.allow
    }
}

/// Where a band host keeps its result and which box's build epoch covers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BandBoxes {
    /// The record's box: the host's own, or for a synthetic host (`wrap_floats`)
    /// the box of the first boxed element it holds (`boxed_elements`).
    pub(crate) slot: BoxId,
    /// The box whose subtree holds every input box of the host: the host's
    /// own, or the parent of its children.
    pub(crate) epoch: BoxId,
}

fn owner(
    boxes: &BoxTree,
    node_id: u64,
    nodes: &[Node],
    cb_node: u64,
) -> Result<(BandBoxes, u64), &'static str> {
    if let Some(own) = boxes.box_of(node_id) {
        let own = BandBoxes {
            slot: own,
            epoch: own,
        };
        return Ok((own, 0));
    }
    // A synthetic host (`band_measured::measured_host`) may hold synthetic
    // children too (runs, leads): it answers by the boxed elements it holds
    // through them, which must be children of one box.
    let mut boxed = Vec::new();
    let mut shape = std::hash::DefaultHasher::new();
    boxed_elements(boxes, nodes, &mut boxed, &mut shape);
    let shape = std::hash::Hasher::finish(&shape);
    // Only inline content (a run around a float): the containing block holds
    // it. Hosts of one block share its record; the walk tells them apart.
    if boxed.is_empty() {
        let cb = boxes.box_of(cb_node).ok_or("no boxed element")?;
        return Ok((BandBoxes { slot: cb, epoch: cb }, shape));
    }
    let first = boxes.box_of(boxed[0]).ok_or("no box")?;
    let mut epoch = None;
    for node in &boxed {
        let parent = boxes.box_of(*node).and_then(|b| boxes.get(b)).ok_or("no box")?.parent;
        if *epoch.get_or_insert(parent) != parent {
            return Err("boxes of different parents");
        }
    }
    let epoch = epoch.filter(|id| boxes.get(*id).is_some()).ok_or("no parent box")?;
    Ok((BandBoxes { slot: first, epoch }, shape))
}

/// The boxed elements of `nodes`, through elements without a box, in order;
/// `shape` hashes the walk, so two synthetic hosts that answer by the same
/// first box never share a record (`BandKey::shape`).
fn boxed_elements(
    boxes: &BoxTree,
    nodes: &[Node],
    out: &mut Vec<u64>,
    shape: &mut std::hash::DefaultHasher,
) {
    use std::hash::Hash;
    nodes.len().hash(shape);
    for node in nodes {
        let e = match node {
            Node::Text(text) => {
                (text.first_source(), text.as_str()).hash(shape);
                continue;
            }
            Node::Element(e) => e,
        };
        if boxes.box_of(e.node_id).is_some() {
            e.node_id.hash(shape);
            out.push(e.node_id);
        } else {
            e.tag.hash(shape);
            boxed_elements(boxes, &e.children, out, shape);
        }
    }
}