//! Static positions retain CSS geometry until the positioned subtree is placed.

use crate::interact::{SpotCell, VT_FRAME, vt_map};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

pub(crate) fn probe(spot: SpotCell, full: bool, child: AnyElement) -> AnyElement {
    Probe { spot, full, child }.into_any_element()
}

struct Probe {
    spot: SpotCell,
    full: bool,
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
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*id),
            size: window.layout_size_unrounded(*id),
        };
        let mut now = self.spot.get();
        now.rotated = VT_FRAME.with(|frame| frame.get()).is_some();
        now.block_strut = self.full;
        now.hole = Some(vt_map(bounds, px(now.line_thickness)));
        self.spot.set(now);
        self.child.prepaint(window, cx);
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
