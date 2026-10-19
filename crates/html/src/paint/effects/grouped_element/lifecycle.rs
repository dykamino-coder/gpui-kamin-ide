//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

use super::painting::paint_body;
use crate::hooks::Hooks;

use super::Grouped;
use crate::paint::effects::{mask_geometry, rectangular_clip};
use gpui::{App, Bounds, LayoutId, Pixels, Window};

impl Hooks for Grouped {
    type Layout = LayoutId;
    type Prepaint = (Bounds<Pixels>, Bounds<Pixels>);

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        // The clip reference box is read BEFORE the child's prepaint: a pure
        // translation of the element is placed into its layout origin there
        // (`Transformed::prepaint`), while `clip_shift` already moves the clip
        // with the transform (CSS Masking §5: clip lives in the element's
        // pre-transform space). Read afterwards, the shift applied twice
        // (clip-transform-order: the clip landed 110px right of the box).
        let clip_bounds = rectangular_clip::reference_box(self, bounds, *_state, window);
        self.child.prepaint(window, cx);
        // Mask positioning box before device snapping (css-masking-1 §7.7).
        let mask_box =
            mask_geometry::positioning_box(self.mask.as_deref(), bounds, *_state, window);
        if let Some(clip) = self.corner_clip.as_mut() {
            clip.prepaint(mask_box, window, cx);
        }
        (clip_bounds, mask_box)
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint_body(self, bounds, _state, _prepaint, window, cx)
    }
}
