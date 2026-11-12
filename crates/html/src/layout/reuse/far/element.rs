//! Elements of a far block: the recording wrapper around a built block and
//! the stand-in that claims its layout subtree (`super`).

use super::{Excess, FarSlot, PendingFar, clear_of, device, ink_beyond, screen_of};
use crate::hooks::{Hooked, Hooks, Wrap};
use crate::layout::reuse::recorder::Recorder;
use crate::layout::reuse::{FlowReuse, Source};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, point, px};
use std::rc::Rc;

/// Transparent wrapper of a built block: captures its layout nodes, checks
/// that its prepaint left nothing but hitboxes and measures its ink. The
/// recording in the making lives in the wrapper, in the frame's element
/// arena; a block that painted far from the screen hands its record to the
/// build (`FlowReuse::add_far`).
pub(in crate::layout::reuse) struct FarRecorder {
    child: AnyElement,
    reuse: Rc<FlowReuse>,
    pending: PendingFar,
    source: Option<Source>,
}

impl FarRecorder {
    pub(in crate::layout::reuse) fn new(
        child: AnyElement,
        reuse: Rc<FlowReuse>,
        pending: PendingFar,
        source: Source,
    ) -> Self {
        Self {
            child,
            reuse,
            pending,
            source: Some(source),
        }
    }

    /// The block's skip record of this frame, the wrap right inside this one
    /// (`FlowCx::record_skip`), stands at every wider flow
    /// (`Record::holds_wider_flows`): its placeholder takes the block there,
    /// and a far record would only keep the block's measure functions for a
    /// claim that never comes (Blink keeps one cached result per object, and
    /// a hit on the size needs nothing else). Only a block whose last skip
    /// wrap gave a record counts (`Store::unrecorded`). The wrapper keeps no
    /// number of its own: thousands of them live in the frame's arena.
    fn skipped_at_other_widths(&mut self) -> bool {
        let Some(id) = self.source.as_ref().map(Source::id) else {
            return false;
        };
        let Some(skip) = self.child.downcast_mut::<Hooked<Recorder>>() else {
            return false;
        };
        let pending = self.reuse.pending.borrow();
        let record = pending.get(skip.0.at());
        !self.reuse.store.unrecorded(id) && record.is_some_and(|record| record.holds_wider_flows())
    }

    /// The recording failed: the block is near (`Store::far_near`).
    fn near(&mut self) {
        if let Some(source) = self.source.take() {
            self.reuse.store.note_near(source.id());
        }
    }
}

impl Wrap for FarRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    /// Transparent to dispatch, as `recorder::Recorder` is.
    fn dispatch_node(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let start = window.layout_request_position();
        let id = self.child.request_layout(window, cx);
        self.pending.layout = window.capture_layout(start, id);
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
        let boxes = self
            .pending
            .layout
            .as_ref()
            .and_then(|layout| window.retained_layout_bounds(layout));
        let scale = window.scale_factor();
        self.pending.placed =
            boxes.map(|boxes| (device(boxes, scale), hits.map(|b| device(b, scale))));
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // Painted after its build was finished (the same tree in a later
        // frame): there is nothing to record.
        if !self.reuse.far_open() {
            self.child.paint(window, cx);
            return;
        }
        // Ink only grows the boxes (`clear_of`): a block whose boxes are not
        // far already is near, and is painted without measuring.
        let screen = screen_of(window);
        let far = self
            .pending
            .placed
            .is_some_and(|(boxes, _)| clear_of(boxes, Excess::default(), screen));
        if !far {
            self.child.paint(window, cx);
            self.near();
            return;
        }
        let callbacks = window.next_frame_callback_count();
        // One scope per block in the frame's scope sink: deferred paint of
        // the block feeds its scope, not the next block's. A block whose
        // boxes lie far from the screen is claimed, not painted, by the next
        // frames: its shaped lines go with this paint (Blink keeps the layout
        // result, not the shaping buffers). If its ink still reaches the
        // screen, the next build shapes them again.
        let child = &mut self.child;
        let ((), scope) = window.with_ink_scope(|window| {
            window.with_line_layouts_released(|window| child.paint(window, cx))
        });
        let (ink, tainted) = (scope.bounds, scope.tainted);
        let scope = scope.id;
        let quiet = !tainted && callbacks == window.next_frame_callback_count();
        let Some(((boxes, hits), layout)) = self
            .pending
            .placed
            .zip(self.pending.layout)
            .filter(|_| quiet)
        else {
            self.near();
            return;
        };
        let ink = match (ink, hits) {
            (Some(a), Some(b)) => Some(a.union(&b)),
            (a, b) => a.or(b),
        };
        let excess: Excess = ink_beyond(boxes, ink);
        if !clear_of(boxes, excess, screen) {
            self.near();
            return;
        }
        if self.skipped_at_other_widths() {
            self.source = None;
            return;
        }
        // Kept for the next pass as their requests made them: the recording
        // frame allocates nothing for the keep. The first pass that claims
        // the block compacts them after its paint.
        window.keep_retained_measures(&layout, false);
        let record = self
            .source
            .take()
            .and_then(|source| self.pending.finish(source, excess, scope));
        if let Some(record) = record {
            self.reuse.add_far(record);
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

    /// A leaf with no prepaint: nothing to dispatch to, as `recorder::Recorder` is.
    fn dispatch_node(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut subtree = self.slot.subtree.get();
        match window.request_retained_layout(&mut subtree) {
            Some(id) => {
                self.slot.subtree.set(subtree);
                self.slot.layout.set(Some(id));
                // The kept leaves make their probes here, where the build of
                // the block would have requested them, unless the widths are
                // those of the last pass (Taffy's cache, no measure).
                if self.slot.prepare {
                    window.prepare_retained_measures(&subtree);
                }
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
        // claim it again with the same functions, shrunk to what a new
        // measure reads (`gpui::CompactMeasure`).
        if self.slot.layout.get().is_some() {
            window.keep_retained_measures(&self.slot.subtree.get(), true);
        }
    }
}
