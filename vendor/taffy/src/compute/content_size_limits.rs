//! Intrinsic probes ignore the measured axis's own min/max size constraints.
use crate::{LayoutInput, RequestedAxis, Size, SizingMode};

/// KaminIDE patch: Resolve content-box min/max limits for an intrinsic measurement.
pub(super) fn for_measurement(
    mut limits: Size<Option<f32>>,
    inputs: LayoutInput,
) -> Size<Option<f32>> {
    if inputs.sizing_mode == SizingMode::IntrinsicSize {
        if inputs.axis != RequestedAxis::Vertical {
            limits.width = None;
        }
        if inputs.axis != RequestedAxis::Horizontal {
            limits.height = None;
        }
    }
    limits
}
