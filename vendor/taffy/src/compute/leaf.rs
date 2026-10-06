//! Computes size using styles and measure functions

#[cfg(feature = "content_size")]
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::style::{AvailableSpace, Overflow, Position};
use crate::tree::{Baselines, CollapsibleMarginSet, RunMode};
use crate::tree::{LayoutInput, LayoutOutput, MeasureOutput, SizingMode};
use crate::util::MaybeMath;
use crate::util::debug::debug_log;
use crate::util::ResolveOrZero;
use crate::{BoxSizing, CoreStyle};
use core::unreachable;

#[path = "leaf_baselines.rs"]
mod measured_baselines;
#[path = "leaf_ratio_size.rs"]
mod ratio_size;
#[path = "leaf_ratio_constraints.rs"]
mod ratio_constraints;

/// Compute the size of a leaf node (node with no children)
pub fn compute_leaf_layout<MeasureFunction, Measurement>(
    inputs: LayoutInput,
    style: &impl CoreStyle,
    resolve_calc_value: impl Fn(*const (), f32) -> f32,
    measure_function: MeasureFunction,
) -> LayoutOutput
where
    MeasureFunction: FnOnce(Size<Option<f32>>, Size<AvailableSpace>) -> Measurement,
    Measurement: Into<MeasureOutput>,
{
    let LayoutInput {
        known_dimensions,
        parent_size,
        available_space,
        sizing_mode,
        run_mode,
        ..
    } = inputs;

    // Note: both horizontal and vertical percentage padding/borders are resolved against the container's inline size (i.e. width).
    // This is not a bug, but is how CSS is specified (see: https://developer.mozilla.org/en-US/docs/Web/CSS/padding#values)
    let margin = style
        .margin()
        .resolve_or_zero(parent_size.width, &resolve_calc_value);
    let padding = style
        .padding()
        .resolve_or_zero(parent_size.width, &resolve_calc_value);
    let border = style
        .border()
        .resolve_or_zero(parent_size.width, &resolve_calc_value);
    let padding_border = padding + border;
    let pb_sum = padding_border.sum_axes();
    let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
        pb_sum
    } else {
        Size::ZERO
    };

    // Resolve node's preferred/min/max sizes (width/heights) against the available space (percentages resolve to pixel values)
    // For ContentSize mode, we pretend that the node has no size styles as these should be ignored.
    let (node_size, node_min_size, node_max_size, aspect_ratio) = match sizing_mode {
        SizingMode::ContentSize => {
            let node_size = known_dimensions;
            let node_min_size = Size::NONE;
            let node_max_size = Size::NONE;
            (node_size, node_min_size, node_max_size, None)
        }
        SizingMode::InherentSize => {
            ratio_constraints::resolve(
                style, parent_size, known_dimensions, box_sizing_adjustment, &resolve_calc_value,
            )
        }
    };

    // Scrollbar gutters are reserved when the `overflow` property is set to `Overflow::Scroll`.
    // However, the axis are switched (transposed) because a node that scrolls vertically needs
    // *horizontal* space to be reserved for a scrollbar
    let scrollbar_gutter = style.overflow().transpose().map(|overflow| match overflow {
        Overflow::Scroll => style.scrollbar_width(),
        _ => 0.0,
    });
    // TODO: make side configurable based on the `direction` property
    let mut content_box_inset = padding_border;
    content_box_inset.right += scrollbar_gutter.x;
    content_box_inset.bottom += scrollbar_gutter.y;

    let has_styles_preventing_being_collapsed_through = !style.is_block()
        || style.overflow().x.is_scroll_container()
        || style.overflow().y.is_scroll_container()
        || style.position() == Position::Absolute
        || style.contain().establishes_independent_formatting_context()
        || padding.top > 0.0
        || padding.bottom > 0.0
        || border.top > 0.0
        || border.bottom > 0.0
        || matches!(node_size.height, Some(h) if h > 0.0)
        || matches!(node_min_size.height, Some(h) if h > 0.0);

    debug_log!("LEAF");
    debug_log!("node_size", dbg:node_size);
    debug_log!("min_size ", dbg:node_min_size);
    debug_log!("max_size ", dbg:node_max_size);

    // Return early if both width and height are known
    if run_mode == RunMode::ComputeSize && has_styles_preventing_being_collapsed_through {
        if let Size {
            width: Some(width),
            height: Some(height),
        } = node_size
        {
            let size = Size { width, height }
                .maybe_clamp(node_min_size, node_max_size)
                .maybe_max(padding_border.sum_axes().map(Some));
            return LayoutOutput {
                size,
                #[cfg(feature = "content_size")]
                scrollable_overflow_rect: Rect::ZERO,
                baselines: Baselines::NONE,
                baselines_x: Baselines::NONE,
                top_margin: CollapsibleMarginSet::ZERO,
                bottom_margin: CollapsibleMarginSet::ZERO,
                margins_can_collapse_through: false,
            };
        };
    }

    // Compute available space
    let available_space = Size {
        width: known_dimensions
            .width
            .map(AvailableSpace::from)
            .unwrap_or(available_space.width)
            .maybe_sub(margin.horizontal_axis_sum())
            .maybe_set(known_dimensions.width)
            .maybe_set(node_size.width)
            .map_definite_value(|size| {
                size.maybe_clamp(node_min_size.width, node_max_size.width)
                    - content_box_inset.horizontal_axis_sum()
            }),
        height: known_dimensions
            .height
            .map(AvailableSpace::from)
            .unwrap_or(available_space.height)
            .maybe_sub(margin.vertical_axis_sum())
            .maybe_set(known_dimensions.height)
            .maybe_set(node_size.height)
            .map_definite_value(|size| {
                size.maybe_clamp(node_min_size.height, node_max_size.height)
                    - content_box_inset.vertical_axis_sum()
            }),
    };

    // Measure node
    let measured: MeasureOutput = measure_function(
        match run_mode {
            RunMode::ComputeSize => known_dimensions,
            RunMode::PerformLayout => Size::NONE,
            RunMode::PerformHiddenLayout => unreachable!(),
        },
        available_space,
    )
    .into();
    let measured_size = measured.size;
    let clamped_size = known_dimensions
        .or(node_size)
        .unwrap_or(measured_size + content_box_inset.sum_axes())
        .maybe_clamp(node_min_size, node_max_size);
    let size = ratio_size::size(
        clamped_size, node_size.height, aspect_ratio, box_sizing_adjustment, node_min_size, node_max_size,
    );
    let size = size.maybe_max(padding_border.sum_axes().map(Some));

    // A scroll container's own padding at the end of the content is part of its scrollable
    // overflow region, so it is included in the overflow rect. Boxes that are not scroll
    // containers do not extend their overflow region by their own padding.
    #[cfg(feature = "content_size")]
    let scrollable_overflow_rect = {
        let is_scroll_container =
            style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container();
        let is_rtl = style.direction().is_rtl();
        let start_padding = if is_rtl { padding.right } else { padding.left };
        let end_padding = if is_rtl { padding.left } else { padding.right };
        Rect {
            left: 0.0,
            right: start_padding
                + measured_size.width
                + if is_scroll_container {
                    end_padding
                } else {
                    0.0
                },
            top: 0.0,
            bottom: padding.top
                + measured_size.height
                + if is_scroll_container {
                    padding.bottom
                } else {
                    0.0
                },
        }
    };

    let baselines = measured_baselines::with_inset(
        style, measured.baseline, measured.last_baseline, content_box_inset.top,
    );
    let baselines_x = measured_baselines::physical_x(style, measured, size.width, content_box_inset);

    LayoutOutput {
        size,
        #[cfg(feature = "content_size")]
        scrollable_overflow_rect,
        baselines,
        baselines_x,
        top_margin: CollapsibleMarginSet::ZERO,
        bottom_margin: CollapsibleMarginSet::ZERO,
        margins_can_collapse_through: !has_styles_preventing_being_collapsed_through
            && size.height == 0.0
            && measured_size.height == 0.0,
    }
}

#[cfg(test)]
mod measured_baseline_tests {
    use super::*;
    use crate::style::{Contain, LengthPercentage};
    type Style = crate::style::Style;

    fn measured() -> MeasureOutput {
        MeasureOutput {
            size: Size {
                width: 40.0,
                height: 90.0,
            },
            baseline: Some(12.0),
            last_baseline: Some(72.0),
            baseline_x: None,
            last_baseline_x: None,
            baseline_x_from_right: false,
        }
    }

    #[test]
    fn leaf_baselines_include_border_and_padding_once() {
        let mut style = Style::default();
        style.padding.top = LengthPercentage::length(7.0);
        style.border.top = LengthPercentage::length(3.0);
        let output = compute_leaf_layout(
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                ..LayoutInput::HIDDEN
            },
            &style,
            |_, _| 0.0,
            |_, _| measured(),
        );
        assert_eq!(output.baselines.first, Some(22.0));
        assert_eq!(output.baselines.last, Some(82.0));
        assert_eq!(output.baselines_x, Baselines::NONE);
    }

    #[test]
    fn layout_containment_suppresses_measured_baselines() {
        let style = Style {
            contain: Contain::LAYOUT,
            ..Style::default()
        };
        let output = compute_leaf_layout(
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                ..LayoutInput::HIDDEN
            },
            &style,
            |_, _| 0.0,
            |_, _| measured(),
        );
        assert_eq!(output.baselines, Baselines::NONE);
    }

    #[test]
    fn native_size_only_measure_callbacks_remain_supported() {
        let output = compute_leaf_layout(
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                ..LayoutInput::HIDDEN
            },
            &Style::default(),
            |_, _| 0.0,
            |_, _| Size {
                width: 40.0,
                height: 90.0,
            },
        );
        assert_eq!(
            output.size,
            Size {
                width: 40.0,
                height: 90.0
            }
        );
        assert_eq!(output.baselines, Baselines::NONE);
    }
}
