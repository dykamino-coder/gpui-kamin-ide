//! Convert measured content baselines to the border box on either physical axis.
//! Keep content order: vertical-rl first/last coordinates can decrease.

use crate::{Baselines, CoreStyle, MeasureOutput, Rect};

pub(super) fn physical_x(
    style: &impl CoreStyle,
    measured: MeasureOutput,
    width: f32,
    inset: Rect<f32>,
) -> Baselines {
    // A completed parent's width may differ from the callback's intrinsic size.
    // Convert right-origin offsets using the final content box, never the probe size.
    let content_width = (width - inset.left - inset.right).max(0.0);
    let offset = |value| {
        if measured.baseline_x_from_right {
            content_width - value
        } else {
            value
        }
    };
    with_inset(
        style,
        measured.baseline_x.map(offset),
        measured.last_baseline_x.map(offset),
        inset.left,
    )
}

pub(super) fn with_inset(
    style: &impl CoreStyle,
    first: Option<f32>,
    last: Option<f32>,
    inset: f32,
) -> Baselines {
    if style.contain().suppresses_baseline() {
        Baselines::NONE
    } else {
        Baselines {
            first: first.map(|value| value + inset),
            last: last.map(|value| value + inset),
        }
    }
}
