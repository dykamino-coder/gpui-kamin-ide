//! The recording wrapper around a built far block (`super`); its stand-in
//! is `stand_in`.

use super::{Excess, PendingFar, clear_of, device, ink_beyond, screen_of};
use crate::hooks::Wrap;
use crate::layout::reuse::recorder::Skip;
use crate::layout::reuse::{FlowReuse, Source};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, point, px};
use std::rc::Rc;

/// Transparent wrapper of a built block: captures its layout nodes, checks
/// that its prepaint left nothing but hitboxes and measures its ink. The
/// recording in the making lives in the wrapper, in the frame's element
/// arena; a block that painted far from the screen hands its record to the
/// build (`FlowReuse::add_far`). A block recorded for a skip as well runs
/// that recording here (`recorder::Skip`): one wrap per block, not two.
pub(in crate::layout::reuse) struct FarRecorder {
    child: AnyElement,
    reuse: Rc<FlowReuse>,
    pending: PendingFar,
    source: Option<Source>,
    /// The block's skip record among the build's (`FlowReuse::pending`).
    skip: Option<u32>,
}

impl FarRecorder {
    pub(in crate::layout::reuse) fn new(
        child: AnyElement,
        reuse: Rc<FlowReuse>,
        (pending, source): (PendingFar, Source),
        skip: Option<usize>,
    ) -> Self {
        Self {
            child,
            reuse,
            pending,
            source: Some(source),
            skip: skip.map(|at| at as u32),
        }
    }

    /// `f` of the child with the block's skip recording, if it has one.
    fn inner<R>(&mut self, f: impl FnOnce(Option<&Skip<'_>>, &mut AnyElement) -> R) -> R {
        let skip = self.skip.map(|at| Skip {
            reuse: &self.reuse,
            at: at as usize,
        });
        f(skip.as_ref(), &mut self.child)
    }

    fn paint_inner(&mut self, window: &mut Window, cx: &mut App) {
        self.inner(|skip, child| match skip {
            Some(skip) => skip.paint(child, window, cx),
            None => child.paint(window, cx),
        });
    }
    /// The block's skip record of this frame, made in this wrap
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
        let Some(at) = self.skip else {
            return false;
        };
        let skip = Skip {
            reuse: &self.reuse,
            at: at as usize,
        };
        !self.reuse.store.unrecorded(id) && skip.holds_wider_flows()
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
        let id = self.inner(|skip, child| match skip {
            Some(skip) => skip.request_layout(child, window, cx),
            None => child.request_layout(window, cx),
        });
        self.pending.layout = window.capture_layout(start, id);
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| {
            self.inner(|skip, child| match skip {
                Some(skip) => skip.prepaint(child, bounds, window, cx),
                None => {
                    child.prepaint(window, cx);
                }
            })
        });
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
            self.paint_inner(window, cx);
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
            self.paint_inner(window, cx);
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
        let ((), scope) = window.with_ink_scope(|window| {
            window.with_line_layouts_released(|window| self.paint_inner(window, cx))
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
