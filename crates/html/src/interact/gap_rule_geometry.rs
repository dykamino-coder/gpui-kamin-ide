//! Gap rules resolve their center and extents before device-pixel snapping.

use super::GapItems;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled, Window, point, px,
};

pub(super) fn probe(items: GapItems) -> AnyElement {
    Probe {
        items,
        child: gpui::canvas(|_, _, _| {}, |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element(),
    }
    .into_any_element()
}

struct Probe {
    items: GapItems,
    child: AnyElement,
}

impl IntoElement for Probe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Probe {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let id = self.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        fallback: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
        let bounds = if window.current_transformation() == gpui::TransformationMatrix::unit() {
            // CSS Gaps 1 centers a rule in the layout gap. A rounded item edge
            // can shift that center even before the rule itself is rounded.
            Bounds {
                origin: window.layout_origin_unrounded(*id),
                size: window.layout_size_unrounded(*id),
            }
        } else {
            fallback
        };
        self.items.borrow_mut().push(bounds);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

pub(super) fn snap(bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    if window.current_transformation() != gpui::TransformationMatrix::unit() {
        return bounds;
    }
    // Resolve inset, intersection and width first; then snap the final strip,
    // as in Blink gap_decorations_painter.cc:476-477 (CSS Gaps 1 §3.1).
    let scale = window.scale_factor();
    let edge = |v: Pixels| px((f32::from(v) * scale + 0.5).floor() / scale);
    Bounds::from_corners(
        point(edge(bounds.left()), edge(bounds.top())),
        point(edge(bounds.right()), edge(bounds.bottom())),
    )
}
