//! Native dimensions are intrinsic contributions, leaving CSS auto axes stretchable.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Refineable, Style, Styled, Window, point,
    px, size,
};

pub(super) fn control(mut paint: Div, natural: [f32; 2]) -> AnyElement {
    let mut style = Style::default();
    style.refine(paint.style());
    // The measured leaf owns CSS sizing; its decoration follows the final box.
    paint.style().margin = Default::default();
    paint.style().min_size = Default::default();
    paint.style().max_size = Default::default();
    paint.style().aspect_ratio = None;
    paint.style().content_box = Some(false);
    NativeControl {
        paint: paint.size_full().into_any_element(),
        style,
        natural,
    }
    .into_any_element()
}

struct NativeControl {
    paint: AnyElement,
    style: Style,
    natural: [f32; 2],
}

impl Element for NativeControl {
    type RequestLayoutState = (LayoutId, LayoutId);
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let paint = self.paint.request_layout(window, cx);
        let [width, height] = self.natural;
        let layout = window.request_measured_layout(self.style.clone(), move |known, _, _, _| {
            size(
                known.width.unwrap_or(px(width)),
                known.height.unwrap_or(px(height)),
            )
        });
        (layout, (paint, layout))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let dimensions = window.layout_size_unrounded(state.1);
        let origin = window.layout_origin_unrounded(state.1);
        window.compute_layout(
            state.0,
            size(
                AvailableSpace::Definite(dimensions.width),
                AvailableSpace::Definite(dimensions.height),
            ),
            cx,
        );
        window.set_layout_root_origin(state.0, origin);
        self.paint.prepaint_at(point(px(0.), px(0.)), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.paint.paint(window, cx);
    }
}

impl IntoElement for NativeControl {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
