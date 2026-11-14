//! A group of sibling blocks as one unit of reuse: the cells of a table row.
//!
//! A table is one child of the root flow, so an edit of one cell makes the
//! whole block differ and every row is built again, although every other row
//! is built from the same inputs. A row has no box of its own in the table's
//! grid: its cells are the grid's children, built one after another, so the
//! unit is the run of elements the row gave (`replay::group`). The owner (the
//! table) names its inputs of the row's build in a key of its own and
//! compares it; the flow adds what any block's record compares: the row's
//! subtree (`Source::same`), the flow key (here the style the table's rows
//! inherit and the constraints of the table's build) and the build counters.
//! Blink skips the layout of such a row (`MaySkipRowLayout`) and Gecko skips
//! a clean row and moves it; neither keeps its paint.

use super::super::replay::{
    Effects, GroupMember, GroupRecorder, GroupSlot, PendingGroup, PendingReplay, ReplayRecorder,
};
use super::Mark;
use super::mark::mark_of;
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::reuse::source::Source;
use crate::layout::reuse::{Counters, FlowKey, FlowReuse};
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::any::Any;
use std::rc::Rc;

/// The scope of the rows of one table of a build.
pub(crate) struct GroupCx {
    reuse: Rc<FlowReuse>,
    flow: Rc<FlowKey>,
    restyled: bool,
}

/// The values of one box without its children, to compare with the box of a
/// later build (a table, a column, a row group: inputs of every row).
pub(crate) struct Shallow(Source);

fn shell(e: &Element) -> Element {
    Element {
        children: Default::default(),
        ..e.clone()
    }
}

impl Shallow {
    pub(crate) fn same(&self, e: &Element) -> bool {
        self.0.same(&shell(e))
    }
}

impl FlowReuse {
    /// The scope of the rows of a table whose rows inherit `inherited` and
    /// are built with `opts`, in a plain build of this document whose root
    /// flow is claimed and settled (a root flow with another key has dropped
    /// every record). The flow key of an equal scope of the last build is
    /// kept, so the records of a table share one.
    pub(crate) fn groups(self: &Rc<Self>, inherited: &BoxStyle, opts: &RenderOpts) -> Option<GroupCx> {
        let cx = &opts.cx;
        let plain = cx.contracts.document_root == Some(self.doc)
            && cx.fragmentation.lines.is_none()
            && cx.clamp.input.is_none()
            && cx.ruby.base.is_none()
            && cx.inline.trim_requests.is_empty()
            && cx.position.oof_identities.is_none();
        if !plain || !self.claimed.get() || !self.store.settled.get() {
            return None;
        }
        let key = FlowKey {
            root: inherited.clone(),
            html: None,
            body: None,
            constraints: cx.constraints.clone(),
            viewport: opts.viewport,
            text: opts.text.clone(),
            normal_line_height: opts.normal_line_height,
            fonts: cx.doc().values.font_generation(),
        };
        let last = self.store.groups.flow.take();
        let flow = match last {
            Some(last) if last.same_outside_widths(&key) && last.same_widths(&key) => last,
            _ => Rc::new(key),
        };
        self.store.groups.flow.replace(Some(flow.clone()));
        Some(GroupCx {
            reuse: self.clone(),
            flow,
            restyled: self.store.restyled.get(),
        })
    }

    pub(crate) fn shallow(&self, e: &Element) -> Shallow {
        Shallow(self.shells.source(&shell(e)))
    }
}

impl GroupCx {
    pub(crate) fn reuse(&self) -> &Rc<FlowReuse> {
        &self.reuse
    }

    /// Push the stand-ins of the elements of group `e` if its record fits:
    /// the flow key, the counters, the owner's key (`same_key`) and the
    /// subtree. `effects` are the owner's buffers of this build.
    pub(crate) fn replay(
        &self,
        e: &Element,
        opts: &RenderOpts,
        same_key: impl FnOnce(&dyn Any) -> bool,
        effects: Rc<dyn Effects>,
        out: &mut Vec<AnyElement>,
    ) -> bool {
        let (reuse, store) = (&self.reuse, &self.reuse.store);
        if !reuse.allow || reuse.again || e.node_id == 0 || !store.groups.open() {
            return false;
        }
        let record = store.groups.records.borrow().get(&e.node_id).cloned();
        let Some(record) = record else { return false };
        let flow = Rc::ptr_eq(&record.flow, &self.flow)
            || record.flow.same_outside_widths(&self.flow)
                && (!record.widths || record.flow.same_widths(&self.flow));
        let fits = flow
            && record.start == Counters::read(&opts.cx)
            && same_key(&*record.key)
            && record.source.same(e);
        if !fits {
            return false;
        }
        if record.widths {
            opts.cx.constraints.note_nested_read();
        }
        record.end.write(&opts.cx);
        let mut next = reuse.groups.next.take();
        next.insert(e.node_id, record.clone());
        reuse.groups.next.replace(next);
        let slot = Rc::new(GroupSlot::new(record, effects));
        super::super::push_cell(&reuse.groups.slots, slot.clone());
        out.extend((0..slot.members()).map(|i| GroupMember::new(slot.clone(), i).hooked()));
        true
    }

    /// The state before the build of a group, and the options to build it
    /// with: a copy of `opts` that records the build's outside width reads
    /// (`track_widths_nested`: the table may be inside a recorded block).
    pub(crate) fn mark(&self, opts: &RenderOpts) -> (Mark, Box<RenderOpts>) {
        let mut mark = mark_of(opts, 0);
        let widths = Rc::<std::cell::Cell<bool>>::default();
        mark.replay = true;
        mark.widths = Some(widths.clone());
        (mark, opts.inner(|cx| cx.constraints.track_widths_nested(widths)))
    }

    /// Wrap the just built elements `out[from..]` of group `e` for a replay
    /// record, if the build changed nothing but the build counters and the
    /// subtree is replayable (as `record` asks of a root-flow block).
    pub(crate) fn record(
        &self,
        e: &Element,
        mark: Mark,
        opts: &RenderOpts,
        key: Rc<dyn Any>,
        effects: Rc<dyn Effects>,
        out: &mut Vec<AnyElement>,
        from: usize,
    ) {
        let now = mark_of(opts, 0);
        let plain = e.node_id != 0
            && from < out.len()
            && now.outlines == mark.outlines
            && now.coordinates == mark.coordinates
            && now.late == mark.late
            && out[from..].iter().all(|el| el.paint_metadata().is_none());
        let Some(widths) = mark.widths.filter(|_| plain && !self.restyled) else {
            return;
        };
        if !super::super::plain::replayable(e, opts.cx.vcx()) {
            return;
        }
        let cell = || {
            let (key, ink) = (self.flow.clone(), &self.reuse.ink);
            Rc::new(PendingReplay::new(key, mark.start, now.start, widths.clone(), false, ink))
        };
        let cells: Box<[Rc<PendingReplay>]> = (from..out.len()).map(|_| cell()).collect();
        let source = self.reuse.shells.source(e);
        let Some(group) = PendingGroup::new(source, key, cells.clone(), effects) else {
            return;
        };
        let group = Rc::new(group);
        let built: Vec<AnyElement> = out.drain(from..).collect();
        out.extend(built.into_iter().zip(cells.iter()).enumerate().map(
            |(index, (child, cell))| {
                let inner = ReplayRecorder::new(child, cell.clone());
                GroupRecorder::new(inner, group.clone(), index).hooked()
            },
        ));
        super::super::push(&self.reuse.groups.pending, group);
    }
}