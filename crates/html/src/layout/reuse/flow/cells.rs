//! The contents of a table cell as a reuse scope of their own.
//!
//! A table is one child of the root flow, so an edit of one cell makes the
//! whole block differ and every cell is built again, although the contents of
//! every other cell are built from the same inputs: the cell's subtree, the
//! style its children inherit (the cell's cascaded style) and the constraints
//! inside the cell. Blink keeps the layout and paint of such clean boxes: a
//! text change dirties its box and the chain of ancestors, and the clean
//! siblings hit their cached result (`layout_box_hot.cc`,
//! `CachedLayoutResult`).
//!
//! The scope is a `FlowCx` whose key holds those inputs in place of the root
//! flow's: the cell's style as the root style, no `html`/`body` boxes, the
//! constraints inside the cell. Its one "child" is the cell, and the build of
//! the cell's contents is the child's build: a record compares the cell's
//! subtree (`Source::same`), the key and the build counters as a root-flow
//! block's record does, and a matching cell gets the stand-in that claims the
//! contents' layout subtree and repeats their prepaint and paint (`replay`).
//! The skip record of a root-flow block (an exact leaf) is not made here: a
//! table sizes its tracks from the contents' intrinsic sizes, which a leaf of
//! one size does not answer. Nor is the far record: it is not compared with
//! the key, which for a root-flow block the store's reset on a new key does.
//!
//! Only contents that build as one element are recorded; a cell cut into
//! columns is built as before.

use super::super::replay::{Replay, ReplaySlot};
use super::mark::mark_of;
use super::{FlowCx, Mark};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::reuse::{Counters, FlowKey, FlowReuse};
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::rc::Rc;

impl FlowReuse {
    /// The scope of the contents of a table cell whose children inherit
    /// `inherited` and are built with `opts`, in a plain build of this
    /// document whose root flow is claimed and settled. A root flow with
    /// another key has dropped every record (`FlowReuse::claim`), a cell's
    /// too, so nothing is recorded under it.
    pub(crate) fn cell(self: &Rc<Self>, inherited: &BoxStyle, opts: &RenderOpts) -> Option<FlowCx> {
        let cx = &opts.cx;
        let plain = cx.contracts.document_root == Some(self.doc)
            && cx.fragmentation.lines.is_none()
            && cx.clamp.input.is_none()
            && cx.ruby.base.is_none()
            && cx.inline.trim_requests.is_empty()
            && cx.position.oof_identities.is_none();
        let settled = self.store.settled.get();
        if !plain || !self.claimed.get() || !settled {
            return None;
        }
        let key = Rc::new(FlowKey {
            root: inherited.clone(),
            html: None,
            body: None,
            constraints: cx.constraints.clone(),
            viewport: opts.viewport,
            text: opts.text.clone(),
            normal_line_height: opts.normal_line_height,
            fonts: cx.doc().values.font_generation(),
        });
        Some(FlowCx {
            reuse: self.clone(),
            key,
            floats: std::cell::Cell::new(false),
            settled,
            resized: false,
            edges: false,
            settled_outside_widths: settled,
            warm: self.store.warm.get(),
            restyled: self.store.restyled.get(),
        })
    }
}

impl FlowCx {
    /// The stand-in of the contents of cell `e` if its replay record fits
    /// (`FlowCx::replay`, without the far record).
    pub(crate) fn replay_contents(&self, e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
        let store = &self.reuse.store;
        if !self.reuse.allow
            || self.reuse.again
            || e.node_id == 0
            || store.banned(e.node_id)
            || store.refused(e.node_id)
        {
            return None;
        }
        let record = store.replays.borrow().get(&e.node_id).cloned();
        let record = record.filter(|r| r.fits(&self.key, Counters::read(&opts.cx), e))?;
        if record.widths {
            opts.cx.constraints.note_nested_read();
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next_replays.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next_replays.replace(next);
        let slot = Rc::new(ReplaySlot::new(record));
        super::super::push_cell(&self.reuse.replay_slots, slot.clone());
        Some(Replay::new(slot).hooked())
    }

    /// The state before the build of the contents of a cell, and the options
    /// to build them with: a copy of `opts` that records the build's outside
    /// width reads for the replay record (`track_widths_nested`: the cell may
    /// be inside a block that is recorded too). A cell is recorded for replay
    /// only, so nothing of the flow's far state is touched.
    pub(crate) fn mark_contents(&self, opts: &RenderOpts) -> (Mark, Box<RenderOpts>) {
        let mut mark = mark_of(opts, 0);
        let widths = Rc::<std::cell::Cell<bool>>::default();
        mark.replay = true;
        mark.widths = Some(widths.clone());
        (mark, opts.inner(|cx| cx.constraints.track_widths_nested(widths)))
    }

    /// Wrap the just built contents of cell `e` for a replay record, if the
    /// build gave exactly one element and changed nothing but the build
    /// counters (as `record` asks of a root-flow block).
    pub(crate) fn record_contents(
        &self,
        e: &Element,
        mark: Mark,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let now = mark_of(opts, out.len());
        let single = e.node_id != 0
            && now.out == mark.out + 1
            && now.outlines == mark.outlines
            && now.coordinates == mark.coordinates
            && now.late == mark.late
            && out.last().is_some_and(|el| el.paint_metadata().is_none());
        if single && !self.restyled {
            self.record_replay(e, mark, now.start, opts, out);
        }
    }
}