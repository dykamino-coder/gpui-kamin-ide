//! Measure replaced grid content without fixing its automatic preferred dimensions.
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, point, px, size,
};

pub(super) struct MeasuredImage {
    pub(super) image: AnyElement,
    pub(super) style: Style,
    pub(super) natural: gpui::Size<Pixels>,
    pub(super) ratio: Option<f32>,
}

impl Element for MeasuredImage {
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
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let picture = self.image.request_layout(window, cx);
        let natural = self.natural;
        let ratio = self.ratio;
        let layout =
            window.request_measured_layout(self.style.clone(), move |known, _, _, _| {
                match (known.width, known.height, ratio) {
                    (Some(w), Some(h), _) => size(w, h),
                    (Some(w), None, Some(r)) => size(w, px(f32::from(w) / r)),
                    (None, Some(h), Some(r)) => size(px(f32::from(h) * r), h),
                    (Some(w), None, None) => size(w, natural.height),
                    (None, Some(h), None) => size(natural.width, h),
                    (None, None, _) => natural,
                }
            });
        (layout, picture)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        picture: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.compute_layout(
            *picture,
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            cx,
        );
        window.set_layout_root_origin(*picture, bounds.origin);
        self.image.prepaint_at(point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _picture: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.image.paint(window, cx);
    }
}

impl IntoElement for MeasuredImage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
