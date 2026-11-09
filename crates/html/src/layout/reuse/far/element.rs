//! Elements of a far block: the recording wrapper around a built block and
//! the stand-in that claims its layout subtree (`super`).

use super::{Excess, FarSlot, PendingFar, clear_of, device, ink_beyond, screen_of};
use crate::hooks::{Hooks, Wrap};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, point, px};
use std::rc::Rc;

/// Transparent wrapper of a built block: captures its layout nodes, checks
/// that its prepaint left nothing but hitboxes and measures its ink.
pub(in crate::layout::reuse) struct FarRecorder {
    child: AnyElement,
    pending: Rc<PendingFar>,
}

impl FarRecorder {
    pub(in crate::layout::reuse) fn new(child: AnyElement, pending: Rc<PendingFar>) -> Self {
        Self { child, pending }
    }
}

impl Wrap for FarRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let start = window.layout_request_position();
        let compacts = self.pending.compacts.clone();
        let id = super::super::compact::with(compacts, window, |window| {
            self.child.request_layout(window, cx)
        });
        self.pending
            .layout
            .replace(window.capture_layout(start, id));
        id
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| self.child.prepaint(window, cx));
        let Some(hits) = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count())
        else {
            return;
        };
        let layout = self.pending.layout.borrow().clone();
        let boxes = layout.and_then(|layout| window.retained_layout_bounds(&layout));
        let scale = window.scale_factor();
        self.pending
            .placed
            .set(boxes.map(|boxes| (device(boxes, scale), hits.map(|b| device(b, scale)))));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = self.pending.sink.clone();
        let callbacks = window.next_frame_callback_count();
        window.with_ink_sink(sink, |window| self.child.paint(window, cx));
        let Some((boxes, hits)) = self.pending.placed.get() else {
            return;
        };
        if self.pending.widths.get()
            || self.pending.sink.tainted()
            || callbacks != window.next_frame_callback_count()
        {
            return;
        }
        let layout = self.pending.layout.borrow().clone();
        let Some(layout) = layout else { return };
        let ink = match (self.pending.sink.bounds(), hits) {
            (Some(a), Some(b)) => Some(a.union(&b)),
            (a, b) => a.or(b),
        };
        let excess: Excess = ink_beyond(boxes, ink);
        if clear_of(boxes, excess, screen_of(window)) {
            self.pending.excess.set(Some(excess));
            self.pending.compacts.compact_all();
            window.keep_retained_measures(&layout);
        }
    }
}

/// Stand-in of a far block: its claimed layout subtree, no prepaint or paint.
pub(in crate::layout::reuse) struct Far {
    slot: Rc<FarSlot>,
}

impl Far {
    pub(in crate::layout::reuse) fn new(slot: Rc<FarSlot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Far {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        match window.request_retained_layout(&self.slot.record.layout) {
            Some(id) => {
                self.slot.layout.set(Some(id));
                (id, ())
            }
            // The nodes are gone: the frame check refuses the block.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        // Painted means the frame check kept the block: the next pass may
        // claim it again with the same functions.
        if self.slot.layout.get().is_some() {
            window.keep_retained_measures(&self.slot.record.layout);
        }
    }
}
