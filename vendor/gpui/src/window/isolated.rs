//! KaminIDE patch: frames that keep the root and redraw only isolated views.
//!
//! An isolated view (`elements/isolated.rs`) is drawn as a deferred draw in a
//! box its parent laid out. Notifying it marks only the view itself dirty
//! (`mark_view_dirty`). If every dirty view lies inside an isolated view, the
//! frame replays the previous root tree as a whole (`reuse_prepaint` and
//! `reuse_paint` over the root's recorded ranges) and redraws the dirty
//! isolated draws in their recorded boxes; the root is neither rendered nor
//! laid out. Upstream GPUI always renders the root and every view on the path
//! to a notified view; this is the relayout boundary that path lacks.

use super::*;

/// What an isolated deferred draw needs to draw its view again alone.
#[derive(Clone)]
pub(crate) struct IsolatedDraw {
    pub(crate) view: AnyView,
    pub(crate) size: Size<Pixels>,
}

/// Where the root tree of the drawn frame sits in its prepaint and paint records.
#[derive(Clone, Default)]
pub(crate) struct RootRanges {
    pub(crate) prepaint: Range<PrepaintStateIndex>,
    pub(crate) paint: Range<PaintIndex>,
}

impl Window {
    /// Lay `view` out alone in `bounds` and draw it after the parent tree,
    /// clipped to the current content mask.
    pub(crate) fn defer_isolated(&mut self, view: AnyView, bounds: Bounds<Pixels>, cx: &mut App) {
        let mut element = view.clone().into_any_element();
        element.layout_as_root(bounds.size.into(), self, cx);
        let content_mask = Some(self.content_mask());
        self.defer_draw(element, bounds.origin, 0, content_mask);
        self.next_frame.isolated_views.insert(view.entity_id());
        if let Some(draw) = self.next_frame.deferred_draws.last_mut() {
            draw.isolated = Some(IsolatedDraw {
                view,
                size: bounds.size,
            });
        }
    }

    /// The previous root tree may be kept: only isolated views changed and
    /// nothing else of the frame is drawn from scratch.
    pub(super) fn kept_root(&self, cx: &App) -> Option<RootRanges> {
        #[cfg(any(feature = "inspector", debug_assertions))]
        if self.inspector.is_some() {
            return None;
        }
        let keep = !self.refreshing
            && !self.dirty_outside_isolated
            && !self.dirty_views.is_empty()
            && self.prompt.is_none()
            && cx.active_drag.is_none()
            && !self.a11y.is_active();
        if keep { self.root_ranges.clone() } else { None }
    }

    /// Replay the kept root tree into the next frame (prepaint half).
    pub(super) fn reuse_root_prepaint(&mut self, kept: &RootRanges, cx: &mut App) {
        cx.entities.extend_accessed(&self.last_accessed_entities);
        self.next_frame.window_control_hitboxes =
            self.rendered_frame.window_control_hitboxes.clone();
        self.reuse_prepaint(kept.prepaint.clone());
    }

    /// A reused isolated draw whose view is dirty gets its element built
    /// again in the recorded box; other draws keep their recorded ranges.
    pub(super) fn rebuild_isolated(&mut self, ix: usize, cx: &mut App) {
        let draw = &self.next_frame.deferred_draws[ix];
        let Some(isolated) = draw.isolated.clone() else {
            return;
        };
        if draw.element.is_some() || !self.dirty_views.contains(&isolated.view.entity_id()) {
            return;
        }
        let rem_size = draw.rem_size;
        self.element_id_stack.clone_from(&draw.element_id_stack);
        self.text_style_stack = draw.text_style_stack.clone();
        let mut element = isolated.view.into_any_element();
        self.with_rem_size(Some(rem_size), |window| {
            element.layout_as_root(isolated.size.into(), window, cx);
        });
        let layout_owner = self.retained_layout();
        let draw = &mut self.next_frame.deferred_draws[ix];
        draw.element = Some(element);
        draw.layout_owner = layout_owner;
    }
}
