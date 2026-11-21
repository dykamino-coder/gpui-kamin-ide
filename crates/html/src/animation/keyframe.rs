//! Keyed CSS effect sampled directly on the document's frame clock.

use super::{
    clock::AnimationTimeSource,
    keyframe_time::{self, LocalTime},
};
use crate::{document::state::DocState, style::computed::AnimSpec};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::rc::Rc;

pub(super) struct Keyframe {
    pub(super) id: ElementId,
    pub(super) spec: AnimSpec,
    pub(super) build: Rc<dyn Fn(f64) -> AnyElement>,
    pub(super) doc: Rc<DocState>,
}

impl Element for Keyframe {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    crate::hooks::keyed_element_identity!();

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let Some(id) = id else {
            let mut child = (self.build)(0.0);
            return (child.request_layout(window, cx), child);
        };
        let frame = self.doc.animation.sample(window, cx);
        let reduced = cx.reduce_motion();
        window.with_element_state::<LocalTime, _>(id, |state, window| {
            let mut state = state.unwrap_or_default();
            let mut elapsed = state.sample(&self.spec, frame);
            if reduced && frame.source == AnimationTimeSource::Live {
                elapsed = if self.spec.infinite {
                    0.0
                } else {
                    keyframe_time::end(&self.spec)
                };
            }
            let requested = keyframe_time::needs_frame(&self.spec, frame, elapsed, reduced);
            self.doc.animation.keyframe(requested);
            if requested {
                self.doc.values.defer_frame();
                window.request_animation_frame();
            }
            let mut child = (self.build)(elapsed);
            ((child.request_layout(window, cx), child), state)
        })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl IntoElement for Keyframe {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
