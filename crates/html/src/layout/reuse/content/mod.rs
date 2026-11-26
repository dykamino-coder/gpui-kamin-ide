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
pub(crate) mod shared;
pub(super) use key::{inert_node, inert_tree};

use super::replay::{PendingReplay, Replay, ReplayRecord, ReplayRecorder, ReplaySlot};
use super::{Counters, FlowReuse, Source};
use crate::dom::{BoxId, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
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
        let (owner, at) = {
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
            (owner, boxes.clock())
        };
        let follows = follows_width(inherited, outer);
        let env = KeyEnv::of(&self.store.inputs, inherited, outer, inner, fresh);
        Some(Content {
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
            // Its width may follow a changed sibling (a table column): the
            // replay would be refused after layout, and the frame built twice.
            epoch <= record.at
                && record.at <= boxes.clock()
                && !boxes.resized_by_siblings(content.owner, record.at)
        };
        // A clipped item whose ink a clip cut is built before layout: its
        // refusal after layout would build the frame twice.
        if !clean
            || !record.key.same(&content.key, record.widths)
            || !record.items.iter().all(|item| item.replays_clipped())
        {
            return None;
        }
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

    /// Wrap the built content in a record (see the module).
    pub(crate) fn record_content(
        &self,
        content: Content,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        // Its width changes with the window's: the next build of another
        // width would not replay it (Blink `ShouldCreateSubsequence` caches
        // only where a replay is likely).
        if content.follows && self.resized.get() {
            return;
        }
        let end = Counters::read(&opts.cx);
        let mut items = Vec::with_capacity(out.len());
        for child in std::mem::take(out) {
            let pending = Rc::new(PendingReplay::new(
                None,
                (content.start, end),
                true,
                content.reads.clone(),
                false,
                &self.ink,
                None,
            ));
            items.push(pending.clone());
            out.push(ReplayRecorder::new(child, pending).hooked());
        }
        let mut pending = self.pending_contents.take();
        pending.push(PendingContent {
            owner: content.owner,
            record: ContentRecord {
                key: content.key,
                widths: false,
                at: content.at,
                start: content.start,
                end,
                items: Box::default(),
            },
            reads: content.reads,
            items,
        });
        self.pending_contents.replace(pending);
    }

    /// The content records of this frame: replayed and finished.
    pub(super) fn finish_contents(&self) {
        let (mut next, mask) = (self.next_contents.take(), self.mask.get());
        for pending in self.pending_contents.take() {
            let PendingContent {
                owner,
                mut record,
                reads,
                items,
            } = pending;
            let finished: Option<Box<[_]>> = items
                .iter()
                .map(|i| i.finish(Source::bare(key(owner)), None, mask).map(Rc::new))
                .collect();
            if let Some(items) = finished {
                record.items = items;
                record.widths = reads.get();
                next.insert(key(owner), Rc::new(record));
            }
        }
        self.store.contents.replace(next);
    }
}
