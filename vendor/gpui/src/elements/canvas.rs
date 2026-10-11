use refineable::Refineable as _;

use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Style, StyleRefinement, Styled, TransformationMatrix, Window, px,
};

/// Construct a canvas element with the given paint callback.
/// Useful for adding short term custom drawing to a view.
pub fn canvas<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint: impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T> {
    Canvas {
        prepaint: Some(Box::new(prepaint)),
        paint: Some(Box::new(paint)),
        style: StyleRefinement::default(),
        unrounded: false,
        layout_id: None,
        unrounded_paint_bounds: None,
    }
}

/// Construct a canvas whose untransformed paint callback uses logical layout bounds.
/// This lets the caller resolve relative geometry before device pixel snapping.
pub fn canvas_with_unrounded_bounds<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint: impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T> {
    let mut canvas = canvas(prepaint, paint);
    canvas.unrounded = true;
    canvas
}

/// A canvas element, meant for accessing the low level paint API without defining a whole
/// custom element
pub struct Canvas<T> {
    prepaint: Option<Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T>>,
    paint: Option<Box<dyn FnOnce(Bounds<Pixels>, T, &mut Window, &mut App)>>,
    style: StyleRefinement,
    unrounded: bool,
    layout_id: Option<LayoutId>,
    unrounded_paint_bounds: Option<Bounds<Pixels>>,
}

impl<T: 'static> IntoElement for Canvas<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<T: 'static> Element for Canvas<T> {
    type RequestLayoutState = Style;
    type PrepaintState = Option<T>;

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
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        self.layout_id = Some(layout_id);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<T> {
        if self.unrounded
            && let Some(layout_id) = self.layout_id
        {
            let raw = Bounds {
                origin: window.layout_origin_unrounded(layout_id),
                size: window.layout_size_unrounded(layout_id),
            };
            // A manually placed canvas may be outside its original layout tree.
            // Only restore the displacement permitted by device origin snapping.
            let half = px(0.5 / window.scale_factor() + 1e-4);
            if (raw.origin.x - bounds.origin.x).abs() <= half
                && (raw.origin.y - bounds.origin.y).abs() <= half
            {
                self.unrounded_paint_bounds = Some(raw);
            }
        }
        // KaminIDE patch: an unrounded canvas hands the same logical bounds to
        // its prepaint callback, so geometry recorded there (collapsed table
        // border probes) can be snapped once, from the exact layout.
        let prepaint_bounds = self.unrounded_paint_bounds.unwrap_or(bounds);
        Some(self.prepaint.take().unwrap()(prepaint_bounds, window, cx))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let prepaint = prepaint.take().unwrap();
        style.paint(bounds, window, cx, |window, cx| {
            let paint_bounds = if window.current_transformation() == TransformationMatrix::unit() {
                self.unrounded_paint_bounds.unwrap_or(bounds)
            } else {
                bounds
            };
            (self.paint.take().unwrap())(paint_bounds, prepaint, window, cx)
        });
    }
}

impl<T> Styled for Canvas<T> {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}
