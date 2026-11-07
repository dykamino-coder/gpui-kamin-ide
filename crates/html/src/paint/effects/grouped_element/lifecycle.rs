//! Жизненный цикл GPUI-элемента; отделён от состояния и конструкторов.

use super::painting::paint_body;
use crate::hooks::Hooks;

use super::Grouped;
use crate::paint::effects::{mask_geometry, rectangular_clip};
use gpui::{App, Bounds, LayoutId, Pixels, Window};

impl Hooks for Grouped {
    type Layout = LayoutId;
    type Prepaint = (Bounds<Pixels>, Bounds<Pixels>);

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        let layout_id = self.child.request_layout_async(window, cx, stack).await;
        (layout_id, layout_id)
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        // The clip reference box is read BEFORE the child's prepaint: a pure
        // translation of the element is placed into its layout origin there
        // (`Transformed::prepaint`), while `clip_shift` already moves the clip
        // with the transform (CSS Masking §5: clip lives in the element's
        // pre-transform space). Read afterwards, the shift applied twice
        // (clip-transform-order: the clip landed 110px right of the box).
        let clip_bounds = rectangular_clip::reference_box(self, bounds, *_state, window);
        self.child.prepaint_async(window, cx, stack).await;
        // Mask positioning box before device snapping (css-masking-1 §7.7).
        let mask_box = mask_geometry::positioning_box(
            self.mask.as_ref().and_then(|m| m.geometry()),
            bounds,
            *_state,
            window,
        );
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
        crate::hooks::paint_on_stack(self, bounds, _state, _prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        _state: &'a mut Self::Layout,
        _prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        paint_body(self, bounds, _state, _prepaint, window, cx, stack)
    }
}
