//! Size-only axis queries keep placement and its geometry untouched.

use super::*;

impl TaffyLayoutEngine {
    #[cfg_attr(feature = "stacker", stacksafe::stacksafe)]
    pub fn measure_size(
        &mut self,
        id: LayoutId,
        available: Size<AvailableSpace>,
        axis: crate::Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        let scale = window.scale_factor();
        let convert = |value| match value {
            AvailableSpace::Definite(value) => taffy::AvailableSpace::Definite(value.0 * scale),
            AvailableSpace::MinContent => taffy::AvailableSpace::MinContent,
            AvailableSpace::MaxContent => taffy::AvailableSpace::MaxContent,
        };
        let axis = match axis {
            crate::Axis::Horizontal => taffy::RequestedAxis::Horizontal,
            crate::Axis::Vertical => taffy::RequestedAxis::Vertical,
        };
        let output = self.taffy.compute_size_with_measure(
            id.into(),
            taffy::Size { width: convert(available.width), height: convert(available.height) },
            axis,
            |input, _, context, style| measurement::leaf(input, context, style, scale, window, cx),
        );
        size(Pixels(output.size.width / scale), Pixels(output.size.height / scale))
    }
}
