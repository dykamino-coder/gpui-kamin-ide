//! Actual content baselines on both physical axes, before native scaling and insets.

use crate::{Pixels, Size};

/// Measured content geometry in logical pixels; baseline order follows content order.
#[derive(Clone, Debug)]
pub struct MeasuredContent {
    /// Content size before border and padding.
    pub size: Size<Pixels>,
    /// First horizontal-text baseline from the content top.
    pub first_y: Option<Pixels>,
    /// Last horizontal-text baseline from the content top.
    pub last_y: Option<Pixels>,
    /// First vertical-text baseline from the selected content origin.
    pub first_x: Option<Pixels>,
    /// Last vertical-text baseline from the selected content origin.
    pub last_x: Option<Pixels>,
    /// X offsets use the final content-box right origin when true.
    pub x_from_right: bool,
    /// Every horizontal-text line baseline in content order, if supplied.
    pub lines_y: Option<Vec<Pixels>>,
    /// Every vertical-text line offset in content order, using the selected X origin.
    pub lines_x: Option<Vec<Pixels>>,
}

impl MeasuredContent {
    /// Content size with no baseline information, for ordinary size callbacks.
    pub fn new(size: Size<Pixels>) -> Self {
        Self {
            size, first_y: None, last_y: None, first_x: None, last_x: None,
            x_from_right: false, lines_y: None, lines_x: None,
        }
    }

    pub(super) fn to_native(&self, scale: f32) -> taffy::MeasureOutput {
        taffy::MeasureOutput {
            size: taffy::Size {
                width: f32::from(self.size.width) * scale,
                height: f32::from(self.size.height) * scale,
            },
            baseline: self.first_y.map(|value| f32::from(value) * scale),
            last_baseline: self.last_y.map(|value| f32::from(value) * scale),
            baseline_x: self.first_x.map(|value| f32::from(value) * scale),
            last_baseline_x: self.last_x.map(|value| f32::from(value) * scale),
            baseline_x_from_right: self.x_from_right,
        }
    }
}

impl From<(Size<Pixels>, Option<Pixels>, Option<Pixels>, Option<Vec<Pixels>>)> for MeasuredContent {
    fn from((size, first_y, last_y, lines_y): (Size<Pixels>, Option<Pixels>, Option<Pixels>, Option<Vec<Pixels>>)) -> Self {
        Self { first_y, last_y, lines_y, ..Self::new(size) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{px, size};

    #[test]
    fn physical_baselines_scale_once_without_reordering_vertical_rl() {
        let measured = MeasuredContent {
            first_x: Some(px(23.0)), last_x: Some(px(11.0)),
            first_y: Some(px(7.0)), last_y: Some(px(17.0)),
            ..MeasuredContent::new(size(px(40.0), px(30.0)))
        };
        let output = measured.to_native(1.25);
        assert_eq!(output.size, taffy::Size { width: 50.0, height: 37.5 });
        assert_eq!(output.baseline_x, Some(28.75));
        assert_eq!(output.last_baseline_x, Some(13.75));
        assert_eq!(output.baseline, Some(8.75));
        assert_eq!(output.last_baseline, Some(21.25));
    }

    #[test]
    fn legacy_horizontal_callback_does_not_synthesize_vertical_baselines() {
        let measured: MeasuredContent = (size(px(40.0), px(30.0)), Some(px(7.0)), Some(px(17.0)), None).into();
        let output = measured.to_native(1.0);
        assert!(!output.baseline_x_from_right);
        assert_eq!(output.baseline_x, None);
        assert_eq!(output.last_baseline_x, None);
        assert_eq!(output.baseline, Some(7.0));
        assert_eq!(output.last_baseline, Some(17.0));
    }

    #[test]
    fn scaled_right_origin_uses_the_completed_parent_width_and_native_insets() {
        let measured = MeasuredContent {
            first_x: Some(px(11.0)), last_x: Some(px(23.0)), x_from_right: true,
            ..MeasuredContent::new(size(px(40.0), px(30.0)))
        };
        let mut style: taffy::Style = taffy::Style::DEFAULT;
        style.padding.left = taffy::LengthPercentage::length(6.25);
        style.padding.right = taffy::LengthPercentage::length(3.75);
        let output = taffy::compute_leaf_layout(
            taffy::LayoutInput {
                known_dimensions: taffy::Size { width: Some(100.0), height: Some(75.0) },
                run_mode: taffy::RunMode::PerformLayout,
                ..taffy::LayoutInput::HIDDEN
            },
            &style,
            |_, _| 0.0,
            |_, _| measured.to_native(1.25),
        );
        // 80 CSSpx final width and 3px right padding, independently of probe width40.
        assert_eq!(output.baselines_x.first, Some(82.5));
        assert_eq!(output.baselines_x.last, Some(67.5));
    }
}
