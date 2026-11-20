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

use super::{DrawPhase, ElementArenaScope, ElementIdContext, ElementStateBox, Window};
use crate::{App, EntityId, GlobalElementId, Subscription};
use collections::FxHashMap;
use std::any::TypeId;

/// Element states moved by reads between frames, taken by the next frame.
#[derive(Default)]
pub(crate) struct ReadStates {
    /// A read is running: element states go to `states`.
    pub(crate) active: bool,
    pub(crate) states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
}

/// Where an element stands in the frame: the ids of its ancestors and its own
/// (the element id stack) and the view rendering it. A pass outside of the
/// draw that builds elements for that place enters it, so their ids, states
/// and view are those the frame gives them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElementPlace {
    ids: ElementIdContext,
    view: Option<EntityId>,
}

impl ElementPlace {
    /// The place of the root element of `view`'s render: the view's id on
    /// the stack, the view rendering.
    pub fn of_view(view: EntityId) -> Self {
        Self {
            ids: ElementIdContext(std::rc::Rc::from([crate::ElementId::View(view)].as_slice())),
            view: Some(view),
        }
    }

}

impl Window {
    /// Snapshot the current element ancestry and rendering view for a later layout step.
    /// KaminIDE patch: embedded hosts must preserve ancestor IDs, not only their own view.
    pub fn element_place(&self) -> ElementPlace {
        ElementPlace {
            ids: self.element_id_context(),
            view: self.rendered_entity_stack.last().copied(),
        }
    }

    /// The element being laid out, prepainted or painted now (its own id on
    /// top of the stack) stands directly under `parent`. Allocates nothing.
    pub fn element_stands_under(&self, parent: &ElementPlace) -> bool {
        let ids = &*parent.ids.0;
        self.rendered_entity_stack.last().copied() == parent.view
            && self.element_id_stack.len() == ids.len() + 1
            && self.element_id_stack[..ids.len()] == *ids
    }

    /// Run `f` at `place`: its id stack and view (the stack is inline up to
    /// its capacity, so entering a place does not allocate).
    fn with_element_place<R>(&mut self, place: &ElementPlace, f: impl FnOnce(&mut Self) -> R) -> R {
        let mut stack: smallvec::SmallVec<[crate::ElementId; 32]> =
            place.ids.0.iter().cloned().collect();
        std::mem::swap(&mut self.element_id_stack, &mut stack);
        let (result, _) = self.with_restore(
            stack,
            |window, outer| std::mem::swap(&mut window.element_id_stack, outer),
            |window| match place.view {
                Some(view) => window.with_rendered_view(view, f),
                None => f(window),
            },
        );
        result
    }

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

    /// Lay out in `f` for the frame that follows, before its draw, at `place`
    /// (`element_place` of the frame that drew the element): elements
    /// go to the app's frame arena (that draw clears it) and layout nodes to
    /// the window's engine, as if `f` ran in the frame's prepaint.
    pub fn with_layout_outside_draw<R>(
        &mut self,
        place: &ElementPlace,
        cx: &mut App,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        let idle = self.invalidator.not_drawing();
        if !idle {
            return f(self, cx);
        }
        self.invalidator.set_phase(DrawPhase::Prepaint);
        let arena = ElementArenaScope::enter(&cx.element_arena);
        let result = self.with_element_place(place, |window| f(window, cx));
        // The frame's own draw clears the arena after painting these elements.
        let _owed_by_the_draw = arena.exit(&cx.element_arena);
        self.invalidator.set_phase(DrawPhase::None);
        result
    }

    /// Lay out in `f` between frames for a read at `place`: elements in a private arena
    /// released on return, an independent layout engine, the line caches
    /// sealed (no growth, no lines lost for the next frame), element states
    /// set aside for the next frame. Only numbers may escape `f`.
    pub fn with_layout_read<R>(
        &mut self,
        place: &ElementPlace,
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
                self.with_nested_layout(|window| window.with_element_place(place, |w| f(w, cx)))
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
