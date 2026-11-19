//! KaminIDE patch: a view drawn in a box its parent reserves and redrawn alone.
//!
//! Notifying an isolated view does not dirty its ancestors. When only isolated
//! views changed, the window keeps the previous frame and redraws just those
//! views at their recorded boxes (`window/isolated.rs`). The box comes from
//! the parent's layout (this element's style and optional sizer child), never
//! from the view's content, so content changes cannot move anything else. The
//! content paints after the parent tree, like a deferred draw, clipped to the
//! parent's content mask: use it for small leaf content nothing overlaps.

use crate::{
    AnyElement, AnyView, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Style, StyleRefinement, Styled, Window,
};
use refineable::Refineable;

/// A box reserved by the parent's layout in which `view` draws on its own.
pub fn isolated(view: impl Into<AnyView>) -> Isolated {
    Isolated {
        view: view.into(),
        style: StyleRefinement::default(),
        sizer: None,
    }
}

/// See [`isolated`].
pub struct Isolated {
    view: AnyView,
    style: StyleRefinement,
    sizer: Option<AnyElement>,
}

impl Isolated {
    /// Size the box as if it held `sizer`; the sizer is laid out, not painted.
    pub fn sizer(mut self, sizer: impl IntoElement) -> Self {
        self.sizer = Some(sizer.into_any_element());
        self
    }
}

impl Styled for Isolated {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for Isolated {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.refine(&self.style);
        let children: Vec<LayoutId> = self
            .sizer
            .as_mut()
            .map(|sizer| sizer.request_layout(window, cx))
            .into_iter()
            .collect();
        (window.request_layout(style, children, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.defer_isolated(self.view.clone(), bounds, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for Isolated {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
#[path = "isolated_tests.rs"]
mod tests;
