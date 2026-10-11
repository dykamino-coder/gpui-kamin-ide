//! Keep complete content line baselines alongside the matching performed native layout.

use super::*;

impl TaffyLayoutEngine {
    pub(super) fn request_measured_layout_with_line_data(
        &mut self,
        style: Style,
        rem_size: Pixels,
        scale_factor: f32,
        mut measure: impl FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> (
            Size<Pixels>,
            Option<Pixels>,
            Option<Pixels>,
            Option<Vec<Pixels>>,
        ) + 'static,
    ) -> LayoutId {
        self.request_measured_layout_with_physical_baselines(
            style, rem_size, scale_factor,
            move |known, available, window, cx| measure(known, available, window, cx).into(),
        )
    }

    /// Measure actual content baseline coordinates on both physical axes.
    pub fn request_measured_layout_with_physical_baselines(
        &mut self,
        style: Style,
        rem_size: Pixels,
        scale_factor: f32,
        measure: impl FnMut(
            Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App,
        ) -> MeasuredContent + 'static,
    ) -> LayoutId {
        self.taffy
            .new_leaf_with_context(
                style.to_taffy(rem_size, scale_factor),
                NodeContext {
                    measure: std::rc::Rc::new(std::cell::RefCell::new(StackSafe::new(Box::new(
                        measure,
                    )))),
                    layout_lines: None,
                    layout_lines_x: None,
                    layout_lines_x_from_right: false,
                },
            )
            .expect(EXPECT_MESSAGE)
            .into()
    }

    pub fn request_measured_layout_with_line_baselines(
        &mut self,
        style: Style,
        rem_size: Pixels,
        scale_factor: f32,
        mut measure: impl FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>, Vec<Pixels>)
        + 'static,
    ) -> LayoutId {
        self.request_measured_layout_with_line_data(
            style,
            rem_size,
            scale_factor,
            move |known, available, window, cx| {
                let (size, first, last, lines) = measure(known, available, window, cx);
                (size, first, last, Some(lines))
            },
        )
    }
}
