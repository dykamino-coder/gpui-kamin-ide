//! KaminIDE patch: host-keyed retained subtrees across frames.
//!
//! Entity views are cached by `Context::notify`; a host engine that owns its
//! own invalidation (a document with dirty bits) needs the same reuse keyed by
//! its inputs instead. The host records the prepaint and paint index ranges of
//! a subtree, stores them in element state (which only survives a frame in
//! which it was accessed), and replays them while every input it keys on is
//! unchanged. GPUI reports which recorded ranges are replayable at all and the
//! window inputs (events, focus) the host cannot observe itself.

use super::*;

/// Prepaint output of a subtree in the frame that recorded it.
#[derive(Clone, Default)]
pub struct RetainedPrepaint(Range<PrepaintStateIndex>);

/// Paint output of a subtree in the frame that recorded it.
#[derive(Clone, Default)]
pub struct RetainedPaint {
    range: Range<PaintIndex>,
    replayable: bool,
}

impl RetainedPaint {
    /// Whether these commands can be copied into a later frame unchanged.
    /// Composition groups live outside the command list (`Scene::groups`) and
    /// isolated scenes are discarded by their owner, so neither replays.
    pub fn replayable(&self) -> bool {
        self.replayable
    }
}

impl Window {
    /// Whether the current element draws in window coordinates without an
    /// enclosing transform, opacity, mask, 3D plane, composition group or
    /// segment capture: replayed commands carry their environment baked in.
    pub fn retained_root_environment(&self) -> bool {
        self.transformation_stack.is_empty()
            && self.element_opacity == 1.0
            && self.mask_map.is_none()
            && self.mask_scale.is_none()
            && self.plane_contexts.is_empty()
            && self.exact_element_offset_depth == 0
            && !self.next_frame.scene.in_group
            && self.next_frame.scene.segment_capture.is_none()
    }

    /// Counter of window inputs that can change element state without the
    /// host's knowledge: dispatched platform events and focus changes.
    pub fn interaction_epoch(&self) -> u64 {
        self.interaction_epoch
    }

    /// Callbacks scheduled for the next frame so far in this draw. A subtree
    /// that adds one (animation frames, deferred notifications) is not settled.
    pub fn next_frame_callback_count(&self) -> usize {
        self.next_frame_callbacks.borrow().len()
    }

    /// Run the prepaint of a subtree and return its range in this frame.
    pub fn record_retained_prepaint<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, RetainedPrepaint) {
        let start = self.prepaint_index();
        let result = f(self);
        (result, RetainedPrepaint(start..self.prepaint_index()))
    }

    /// Copy the prepaint output recorded in the previous frame into this one.
    /// The caller guarantees the record comes from the rendered frame.
    pub fn replay_retained_prepaint(&mut self, record: &RetainedPrepaint) -> RetainedPrepaint {
        let start = self.prepaint_index();
        self.reuse_prepaint(record.0.clone());
        RetainedPrepaint(start..self.prepaint_index())
    }

    /// Whether a record made in the rendered frame can be replayed into this
    /// one. Deferred draws paint after the whole tree, outside the subtree's
    /// paint range, and nested deferred draws are appended after their owner;
    /// when the subtree registered any, every deferred draw from its first one
    /// on must be free of composition groups.
    pub fn retained_replayable(&self, prepaint: &RetainedPrepaint, paint: &RetainedPaint) -> bool {
        use crate::scene::{PaintOperation, Primitive};
        let frame = &self.rendered_frame;
        let first = prepaint.0.start.deferred_draws_index;
        let groups = |draw: &DeferredDraw| {
            let range = draw.paint_range.start.scene_index..draw.paint_range.end.scene_index;
            frame.scene.paint_operations[range].iter().any(|operation| {
                matches!(
                    operation,
                    PaintOperation::Primitive(Primitive::Surface(surface)) if surface.group > 0
                )
            })
        };
        paint.replayable
            && (first == prepaint.0.end.deferred_draws_index
                || !frame.deferred_draws[first..].iter().any(groups))
    }

    /// Run the paint of a subtree and return its range in this frame.
    pub fn record_retained_paint<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> (R, RetainedPaint) {
        let root = self.retained_root_environment();
        let groups = self.next_frame.scene.groups.len();
        let start = self.paint_index();
        let result = f(self);
        let replayable = root && groups == self.next_frame.scene.groups.len();
        (result, RetainedPaint { range: start..self.paint_index(), replayable })
    }

    /// Copy replayable paint output recorded in the previous frame into this one.
    /// The caller guarantees the record comes from the rendered frame.
    pub fn replay_retained_paint(&mut self, record: &RetainedPaint) -> RetainedPaint {
        debug_assert!(record.replayable, "non-replayable paint record");
        let start = self.paint_index();
        self.reuse_paint(record.range.clone());
        RetainedPaint { range: start..self.paint_index(), replayable: true }
    }
}
