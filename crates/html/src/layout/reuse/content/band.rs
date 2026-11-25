//! The key of a float band host's retained layout result (`band_flow::HostSlot`).
//!
//! A band host's plan (the places and sizes of its children around its
//! floats) is a function of its subtree, the containing-block width it is
//! measured at, and what its build reads from outside the subtree: the same
//! environment a content record keys on (`KeyEnv`), the host's identity, the
//! counters its build starts from, the viewport and the image revision (a
//! decoded image changes an intrinsic size without a box epoch). The width is
//! the measure's own input and keys each kept plan; the subtree is clean while
//! the host's build epoch is not newer than the publication of the record
//! (`BoxTree::build_epoch`). Blink keys `LayoutResult` by the
//! `ConstraintSpace` the same way; the epoch replaces its dirty bits.

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
    start: Counters,
    viewport: (f32, f32),
    images: u64,
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
            _ if self.start != other.start => Some("counters"),
            _ if self.viewport != other.viewport => Some("viewport"),
            _ if self.images != other.images => Some("images"),
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
            start: Counters::zero(),
            viewport: (width, 0.0),
            images: 0,
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
        let (owner, seen) = {
            let mut boxes = opts.cx.doc().boxes.try_borrow_mut().ok()?;
            let Some(owner) = owner(&boxes, node_id, nodes) else {
                crate::why::note("band_slot.none", || "no box answers for the host".into());
                return None;
            };
            let seen = boxes
                .slot_mut(owner.slot)
                .map(|s| *s.band_seen.get_or_insert(frame));
            (owner, seen)
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
            start: Counters::read(&opts.cx),
            viewport: vcx.viewport,
            images: vcx.doc.images.borrow().snapshot().revision(),
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
    /// the box of its first element child, one per host.
    pub(crate) slot: BoxId,
    /// The box whose subtree holds every input box of the host: the host's
    /// own, or the parent of its children.
    pub(crate) epoch: BoxId,
}

fn owner(boxes: &BoxTree, node_id: u64, nodes: &[Node]) -> Option<BandBoxes> {
    if let Some(own) = boxes.box_of(node_id) {
        return Some(BandBoxes {
            slot: own,
            epoch: own,
        });
    }
    let first = nodes.iter().find_map(|n| match n {
        Node::Element(e) => boxes.box_of(e.node_id),
        _ => None,
    })?;
    let epoch = boxes.owner_of(nodes)?;
    Some(BandBoxes { slot: first, epoch })
}
