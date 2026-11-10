//! Elements of a far block: the recording wrapper around a built block and
//! the stand-in that claims its layout subtree (`super`).

use super::{Excess, FarSlot, PendingFar, clear_of, device, ink_beyond, screen_of};
use crate::hooks::{Hooks, Wrap};
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
        let Some(sink) = self.reuse.far_sink() else {
            self.child.paint(window, cx);
            return;
        };
        // One sink per build, one scope per block: deferred paint of the
        // block feeds its scope, not the next block's (`InkSink::begin_scope`).
        let scope = sink.begin_scope();
        let callbacks = window.next_frame_callback_count();
        // A block whose boxes lie far from the screen is claimed, not
        // painted, by the next frames: its shaped lines go with this paint
        // (Blink keeps the layout result, not the shaping buffers). If its
        // ink still reaches the screen, the next build shapes them again.
        let screen = screen_of(window);
        let far = self
            .pending
            .placed
            .is_some_and(|(boxes, _)| clear_of(boxes, Excess::default(), screen));
        let child = &mut self.child;
        window.with_ink_sink(sink.clone(), |window| match far {
            true => window.with_line_layouts_released(|window| child.paint(window, cx)),
            false => child.paint(window, cx),
        });
        let (ink, tainted) = (sink.bounds(), sink.tainted());
        let quiet = !tainted && callbacks == window.next_frame_callback_count();
        let Some(((boxes, hits), layout)) = self
            .pending
            .placed
            .zip(self.pending.layout.as_ref())
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
        // Kept for the next pass as their requests made them: the recording
        // frame allocates nothing for the keep. The first pass that claims
        // the block compacts them after its paint.
        window.keep_retained_measures(layout, false);
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

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        match window.request_retained_layout(&self.slot.subtree) {
            Some(id) => {
                self.slot.layout.set(Some(id));
                // The kept leaves make their probes here, where the build of
                // the block would have requested them, unless the widths are
                // those of the last pass (Taffy's cache, no measure).
                if self.slot.prepare {
                    window.prepare_retained_measures(&self.slot.subtree);
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
            window.keep_retained_measures(&self.slot.subtree, true);
        }
    }
}
