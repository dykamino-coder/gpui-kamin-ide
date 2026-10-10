//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

mod painting;
use painting::paint_body;

use super::Grouped;
use crate::paint::effects::{mask_geometry, rectangular_clip};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window,
};

impl Element for Grouped {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (Bounds<Pixels>, Bounds<Pixels>);

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
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // The clip reference box is read BEFORE the child's prepaint: a pure
        // translation of the element is placed into its layout origin there
        // (`Transformed::prepaint`), while `clip_shift` already moves the clip
        // with the transform (CSS Masking §5: clip lives in the element's
        // pre-transform space). Read afterwards, the shift applied twice
        // (clip-transform-order: the clip landed 110px right of the box).
        let clip_bounds = rectangular_clip::reference_box(self, bounds, *_state, window);
        self.child.as_mut().unwrap().prepaint(window, cx);
        // Mask positioning box before device snapping (css-masking-1 §7.7).
        let mask_box =
            mask_geometry::positioning_box(self.mask.as_deref(), bounds, *_state, window);
        (clip_bounds, mask_box)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint_body(
            self,
            _id,
            _inspector_id,
            bounds,
            _state,
            _prepaint,
            window,
            cx,
        )
    }
}
