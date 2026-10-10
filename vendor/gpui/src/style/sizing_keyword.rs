//! Preserve intrinsic CSS dimensions until the native layout measures content.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A content-based width or height which cannot be represented by a length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum CssSizingKeyword {
    /// Use the smallest intrinsic size.
    MinContent,
    /// Use the unwrapped intrinsic size.
    MaxContent,
    /// Clamp available space between the intrinsic minimum and maximum.
    FitContent,
    /// Replace available space with a logical pixel argument.
    FitContentPixels(f32),
    /// Replace available space with a fraction of the containing block.
    FitContentPercent(f32),
}

impl CssSizingKeyword {
    pub(crate) fn to_native(self, scale: f32) -> taffy::Dimension {
        match self {
            Self::MinContent => taffy::Dimension::min_content(),
            Self::MaxContent => taffy::Dimension::max_content(),
            Self::FitContent => taffy::Dimension::fit_content(),
            Self::FitContentPixels(value) => taffy::Dimension::fit_content_px(value * scale),
            Self::FitContentPercent(value) => taffy::Dimension::fit_content_percent(value),
        }
    }
}
