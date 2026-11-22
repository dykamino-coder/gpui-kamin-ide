//! Hover transitions sample the document's shared animation frame.

mod motion;

use super::clock::AnimationTimeSource;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};
use motion::Motion;
use std::rc::Rc;

/// Rebuilds the actual subtree from its current hover progress.
pub struct Transition {
    id: ElementId,
    seconds: f32,
    build: Rc<dyn Fn(f32) -> AnyElement>,
    doc: Rc<crate::document::state::DocState>,
}

impl Transition {
    pub fn new(
        id: ElementId,
        seconds: f32,
        build: Rc<dyn Fn(f32) -> AnyElement>,
        doc: Rc<crate::document::state::DocState>,
    ) -> Self {
        Self {
            id,
            seconds: seconds.max(0.01),
            build,
            doc,
        }
    }
}

impl Element for Transition {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;

    crate::hooks::keyed_element_identity!();

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let Some(global_id) = id else {
            let mut child = (self.build)(0.0);
            return (child.request_layout(window, cx), child);
        };
        let frame = self.doc.animation.sample(window, cx);
        let reduced = cx.reduce_motion();
        window.with_element_state::<Motion, _>(global_id, |state, window| {
            let mut state = state.unwrap_or_default();
            let active = state.sample(frame, self.seconds, reduced);
            let requested = active && frame.source == AnimationTimeSource::Live;
            self.doc.animation.transition(active, requested);
            if requested {
                self.doc.values.defer_frame();
                window.request_animation_frame();
            }
            let mut child = (self.build)(state.progress);
            let layout = child.request_layout(window, cx);
            ((layout, child), state)
        })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        crate::interactive::hitbox::prepaint_hitbox(child, bounds, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
        let hovered = hitbox.is_hovered(window);
        let Some(global_id) = id else { return };
        let frame = self.doc.animation.sample(window, cx);
        // Hitboxes establish the target after layout. Its event wakes one frame,
        // including for a stopped snapshot; only live progress keeps scheduling.
        window.with_element_state::<Motion, _>(global_id, |state, window| {
            let mut state = state.unwrap_or_default();
            if state.target(hovered, frame) {
                self.doc.animation.hover_changed();
                self.doc.values.defer_frame();
                window.request_animation_frame();
            }
            ((), state)
        });
    }
}

impl IntoElement for Transition {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
