//! KaminIDE patch: the document step before a frame and layout passes
//! between frames.
//!
//! A host lays out its content before `Window::draw` paints it (HTML "update
//! the rendering": style and layout run before paint, and synchronous
//! geometry reads of scripts run between frames). Two passes:
//! - the frame step (`Window::with_layout_outside_draw`), run by the
//!   observers of `Window::on_update_rendering` at the start of every draw:
//!   its elements, layout nodes, shaped lines and element states belong to
//!   the frame that follows, exactly as if they were made in its prepaint;
//! - a read (`Window::with_layout_read`), between frames and without one:
//!   nothing it makes outlives it except the element states it moved, which
//!   the next frame takes before those of the drawn frame.

use super::{DrawPhase, ElementArenaScope, ElementStateBox, Window};
use crate::{App, GlobalElementId, Subscription};
use collections::FxHashMap;
use std::any::TypeId;

/// Element states moved by reads between frames, taken by the next frame.
#[derive(Default)]
pub(crate) struct ReadStates {
    /// A read is running: element states go to `states`.
    pub(crate) active: bool,
    pub(crate) states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
}

impl Window {
    /// Register `f` to run at the start of every draw, before its paint pass
    /// (outside of the draw phases): the host's step that styles and lays
    /// out its content for the frame (`with_layout_outside_draw`).
    pub fn on_update_rendering(
        &self,
        mut f: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.update_rendering_observers.insert(
            (),
            Box::new(move |window, cx| {
                f(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    pub(crate) fn update_rendering(&mut self, cx: &mut App) {
        self.update_rendering_observers
            .clone()
            .retain(&(), |observer| observer(self, cx));
    }

    /// Lay out in `f` for the frame that follows, before its draw: elements
    /// go to the app's frame arena (that draw clears it) and layout nodes to
    /// the window's engine, as if `f` ran in the frame's prepaint.
    pub fn with_layout_outside_draw<R>(
        &mut self,
        cx: &mut App,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        let idle = self.invalidator.not_drawing();
        if !idle {
            return f(self, cx);
        }
        self.invalidator.set_phase(DrawPhase::Prepaint);
        let arena = ElementArenaScope::enter(&cx.element_arena);
        let result = f(self, cx);
        // The frame's own draw clears the arena after painting these elements.
        let _owed_by_the_draw = arena.exit(&cx.element_arena);
        self.invalidator.set_phase(DrawPhase::None);
        result
    }

    /// Lay out in `f` between frames for a read: elements in a private arena
    /// released on return, an independent layout engine, the line caches
    /// sealed (no growth, no lines lost for the next frame), element states
    /// set aside for the next frame. Only numbers may escape `f`.
    pub fn with_layout_read<R>(
        &mut self,
        cx: &mut App,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        assert!(
            self.invalidator.not_drawing() && !self.read_states.active,
            "a layout read runs between frames"
        );
        self.invalidator.set_phase(DrawPhase::Prepaint);
        self.read_states.active = true;
        let result = super::with_temporary_element_arena(|| {
            crate::text_system::with_layouts_sealed(|| {
                self.with_nested_layout(|window| f(window, cx))
            })
        });
        self.read_states.active = false;
        self.invalidator.set_phase(DrawPhase::None);
        result
    }

    /// Bounds of every laid out node of the subtree `root` in the active
    /// engine, depth first, relative to the layout root: two passes placed a
    /// subtree the same if they give equal lists.
    pub fn subtree_layout_bounds(&mut self, root: crate::LayoutId) -> Vec<crate::Bounds<crate::Pixels>> {
        let scale = self.scale_factor();
        self.layout_engine
            .as_mut()
            .map(|engine| engine.subtree_bounds(root, scale))
            .unwrap_or_default()
    }

    /// What layout passes leave between frames: values in the app's frame
    /// arena, line layouts in the frame caches, element states set aside by
    /// reads for the next frame.
    pub fn layout_footprint(&self, cx: &App) -> (usize, usize, usize) {
        (
            cx.element_arena.borrow().allocations(),
            self.text_system().cached_line_layouts(),
            self.read_states.states.len(),
        )
    }
}
