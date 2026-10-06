//! Measured content size and physical baseline coordinates before leaf insets.

use crate::geometry::Size;

/// KaminIDE leaf callback adapter: measured content size and physical-axis
/// baseline offsets. The leaf algorithm adds its content-box inset before
/// putting them into LayoutOutput; these must not be border-box offsets.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MeasureOutput {
    /// Measured content-box size before the leaf adds padding and border.
    pub size: Size<f32>,
    /// First text baseline measured from the content-box top.
    pub baseline: Option<f32>,
    /// Last text baseline measured from the content-box top.
    pub last_baseline: Option<f32>,
    /// First vertical-text baseline offset on the physical X axis.
    pub baseline_x: Option<f32>,
    /// Last vertical-text baseline offset on the physical X axis.
    pub last_baseline_x: Option<f32>,
    /// X offsets are measured from the final content-box right when true.
    pub baseline_x_from_right: bool,
}

impl From<Size<f32>> for MeasureOutput {
    fn from(size: Size<f32>) -> Self {
        Self {
            size,
            baseline: None,
            last_baseline: None,
            baseline_x: None,
            last_baseline_x: None,
            baseline_x_from_right: false,
        }
    }
}

