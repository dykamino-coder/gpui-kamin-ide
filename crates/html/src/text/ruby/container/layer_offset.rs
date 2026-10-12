//! Move an outer annotation beyond descendant ruby containers, CSS Ruby 1 §3.2.1.

use crate::text::ruby::layers::Layers;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, point, px,
};
use std::rc::Rc;

pub(super) fn offset(child: AnyElement, layers: &[Rc<Layers>], under: bool) -> AnyElement {
    Offset {
        child,
        layers: layers
            .iter()
            .map(|layer| (layer.clone(), layer.last_index()))
            .collect(),
        under,
    }
    .into_any_element()
}

struct Offset {
    child: AnyElement,
    layers: Vec<crate::text::ruby::layers::LayerAttachment>,
    under: bool,
}

impl Element for Offset {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let h = self
            .layers
            .iter()
            .map(|(layer, before)| layer.before(self.under, *before, window))
            .fold(0.0f32, f32::max);
        let shift = point(px(0.0), px(if self.under { h } else { -h }));
        window.with_exact_element_offset(shift, |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for Offset {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
