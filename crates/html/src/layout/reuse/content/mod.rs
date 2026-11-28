//! Replay of a box's content at any depth without building its elements.
//!
//! The content of a block container (`render::blocks::stack`: the children of a cell,
//! a block, a list item) is built from the box's subtree and from what the
//! container gives it from outside: the container's style, constraints, the
//! document environment and the build counters. If no box of the subtree changed
//! since the recording frame (the build epoch `Epochs::paint` is not newer than the record) and the
//! outside input is the same, a build would give the same elements: each output element
//! replays its layout nodes, prepaint and paint of the previous frame
//! (`replay`), and the frame check (`FlowReuse::validate_replays`) refuses
//! a replay that landed elsewhere or needed a new measure.
//!
//! The box tree decides cleanliness in O(1) per container, without comparing the
//! subtree: a DOM edit (from script too) is published as an epoch record, and
//! the content on the path to it is built while the rest replays. So
//! LayoutNG takes the cached `LayoutResult` of a clean box, and Blink
//! `PaintController` the paint subsequence of a clean subtree.

pub(super) mod inert;
mod key;
mod record;
pub(crate) mod shared;
pub(super) use key::{inert_node, inert_tree, key as box_key};

use super::replay::{PendingReplay, Replay, ReplayRecord, ReplaySlot};
use super::{Counters, FlowReuse};
use crate::dom::{BoxId, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::resources::images::store::ImageRead;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use key::{ContentKey, KeyEnv, follows_width, key, plain};
use std::cell::Cell;
use std::rc::Rc;

/// A content record: the input, the publication number and the elements of the last frame.
pub(crate) struct ContentRecord {
    key: ContentKey,
    /// The build read an outside width (`BoxConstraints::track_widths_below`): same widths only.
    widths: bool,
    /// `BoxTree::clock` of the recording build: valid while the box's build epoch is not newer.
    at: u32,
    start: Counters,
    end: Counters,
    items: Box<[Rc<ReplayRecord>]>,
    /// The images its build read, in the state read (`ImageStore::replay_reads`).
    images: Box<[ImageRead]>,
}

/// A content built in this build with record wrappers.
pub(super) struct PendingContent {
    owner: BoxId,
    record: ContentRecord,
    reads: Rc<Cell<bool>>,
    items: Vec<Rc<PendingReplay>>,
}

/// The content of container `inherited` in this build: the key for replay and record.
pub(crate) struct Content {
    owner: BoxId,
    key: ContentKey,
    at: u32,
    start: Counters,
    /// Outside width reads of the build (`track`).
    reads: Rc<Cell<bool>>,
    /// The container's width follows the outside one (`key::follows_width`).
    follows: bool,
    /// The image request log before its build (`ImageStore::log_position`).
    images_at: usize,
}

impl Content {
    /// Record the outside width reads of the content's build in `inner`.
    pub(crate) fn track(&self, inner: &mut RenderOpts) {
        inner.cx.constraints.track_widths_below(self.reads.clone());
    }
}

impl FlowReuse {
    /// The content `nodes` of container `inherited` under the children options `inner`
    /// (`outer`: the container's options), if it is worth replaying or recording.
    pub(crate) fn content(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        outer: &RenderOpts,
        inner: &RenderOpts,
        fresh: bool,
    ) -> Option<Content> {
        // The root flow has its own records (`FlowCx`).
        let root = match self.root {
            super::Root::Body { html, body } => {
                inherited.self_node == body || Some(inherited.self_node) == html
            }
            super::Root::TopLevel => inherited.reset_context.icb,
        };
        if root
            || nodes.is_empty()
            || outer.cx.contracts.document_root != Some(self.doc)
            || !plain(nodes, inherited, inner, &self.inert)
        {
            return None;
        }
        let (owner, at, distributed) = {
            let boxes = inner.cx.doc().boxes.borrow();
            let owner = boxes.owner_of(nodes)?;
            // A block of the root flow is replayed whole by its records
            // (`FlowCx`), whose recorders hold no content recorder.
            let parent = boxes.get(owner)?.parent;
            let flow = match self.root {
                super::Root::Body { body, .. } => boxes.box_of(body),
                super::Root::TopLevel => Some(crate::dom::BoxId::NONE),
            };
            if flow == Some(parent) || boxes.get(parent).is_none() {
                return None;
            }
            (owner, boxes.clock(), boxes.distributed(owner))
        };
        let follows = follows_width(inherited, outer, distributed);
        let env = KeyEnv::of(&self.store.inputs, inherited, outer, inner, fresh);
        // The log holds this build's reads only: the first content empties it.
        let first = !self.images_log.replace(true);
        let images_at = inner.cx.vcx().doc.image_log_mark(first);
        Some(Content {
            images_at,
            follows,
            owner,
            at,
            start: Counters::read(&inner.cx),
            key: ContentKey {
                env: self.store.inputs.env(&env),
                ids: key::ids(inherited),
                len: u32::try_from(nodes.len()).unwrap_or(u32::MAX),
                widths: inner.cx.constraints.key_widths(),
            },
            reads: Rc::new(Cell::new(follows)),
        })
    }

    /// Replay the content if its record is valid (see the module).
    pub(crate) fn replay_content(
        &self,
        content: &Content,
        opts: &RenderOpts,
    ) -> Option<Vec<AnyElement>> {
        if !self.allow || self.again || self.store.banned(key(content.owner)) {
            return None;
        }
        let record = self
            .store
            .contents
            .borrow()
            .get(&key(content.owner))
            .cloned()?;
        let clean = {
            let boxes = opts.cx.doc().boxes.borrow();
            let epoch = boxes.build_epoch(content.owner)?;
            // A width following a changed sibling (a column) is refused after layout.
            epoch <= record.at
                && record.at <= boxes.clock()
                && !boxes.resized_by_siblings(content.owner, record.at)
        };
        let cut = !record
            .items
            .iter()
            .all(|item| item.replays_clipped() && item.hovers_hold(&self.hover));
        let images = || opts.cx.vcx().doc.replay_image_reads(&record.images);
        if !clean || cut || !record.key.same(&content.key, record.widths) || !images() {
            crate::work::add!(ContentsRefusedBeforeLayout, 1);
            return None;
        }
        crate::work::add!(ContentsReplayed, 1);
        if record.widths {
            // The replayed output read the widths: so does this build, for
            // the records of the subtrees around it.
            opts.cx.constraints.cb_width();
            opts.cx.constraints.avail_w();
        }
        content
            .start
            .advanced(record.start, record.end)
            .write(&opts.cx);
        let out = record
            .items
            .iter()
            .map(|item| {
                let slot = Rc::new(ReplaySlot::new(item.clone()));
                super::push_cell(&self.replay_slots, slot.clone());
                Replay::new(slot).hooked()
            })
            .collect();
        let mut next = self.next_contents.take();
        next.insert(key(content.owner), record);
        self.next_contents.replace(next);
        Some(out)
    }
}
