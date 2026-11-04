//! Измерение листа приостанавливает обход перед вложенной раскладкой.
use super::*;
pub(in crate::taffy) async fn leaf_async(
    input: LayoutInput,
    mut context: Option<&mut NodeContext>,
    style: &Style,
    scale: f32,
    window: &mut Window,
    cx: &mut App,
    stack: &mut crate::PaintStack,
) -> LayoutOutput {
    let performed = matches!(input.run_mode, taffy::RunMode::PerformLayout);
    if performed {
        if let Some(context) = context.as_deref_mut() {
            context.static_positions.clear();
            context.layout_lines = None;
            context.layout_lines_x = None;
            context.layout_lines_x_from_right = false;
        }
    }
    taffy::compute_leaf_layout_async(
        input,
        style,
        taffy::tree::calc_value,
        async |known, available| {
            crate::frame_perf::LAYOUT_MEASURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _timer = super::MeasureTimer(std::time::Instant::now());
            let Some(context) = context else {
                return taffy::Size::default().into();
            };
            let known = Size {
                width: known.width.map(|value| Pixels(value / scale)),
                height: known.height.map(|value| Pixels(value / scale)),
            };
            let from_native = |value| match value {
                taffy::AvailableSpace::Definite(value) => {
                    AvailableSpace::Definite(Pixels(value / scale))
                }
                taffy::AvailableSpace::MinContent => AvailableSpace::MinContent,
                taffy::AvailableSpace::MaxContent => AvailableSpace::MaxContent,
            };
            let available = size(from_native(available.width), from_native(available.height));
            let mut geometry = super::MeasuredBox::from_native(input.parent_size, style, scale);
            geometry.intrinsic_axis = if input.run_mode == taffy::RunMode::ComputeSize {
                match input.axis {
                    taffy::RequestedAxis::Horizontal => Some(crate::Axis::Horizontal),
                    taffy::RequestedAxis::Vertical => Some(crate::Axis::Vertical),
                    taffy::RequestedAxis::Both => None,
                }
            } else {
                None
            };
            if performed {
                geometry.fixed = Size {
                    width: input.known_dimensions.width.is_some(),
                    height: input.known_dimensions.height.is_some(),
                };
            }
            let measured = context
                .measure
                .borrow_mut()
                .measure(known, available, geometry, window, cx, stack)
                .await;
            let output = measured.to_native(scale);
            // Intrinsic probes must not replace metadata belonging to a cached full layout.
            if performed {
                context.static_positions = measured.static_positions;
                context.layout_lines = measured.lines_y;
                context.layout_lines_x = measured.lines_x;
                context.layout_lines_x_from_right = measured.x_from_right;
            }
            output
        },
    )
    .await
}
