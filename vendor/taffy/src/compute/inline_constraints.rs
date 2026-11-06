//! Atomic inline used sizes precede layout of their independent formatting context.
use super::common::box_constraints;
use crate::{CoreStyle, LayoutContainingBlock, LayoutInput, MaybeMath, NodeId, OofItemStyle, RequestedAxis, RunMode};

/// CSS 2 §10.3.9/10.6.6: select the atomic box's size without a synthetic parent.
/// Definite preferred sizes need no intrinsic pass. Content-derived block sizes
/// remain indefinite for percentages even though final layout receives a size.
pub(super) fn resolve(
    tree: &mut impl LayoutContainingBlock,
    node: NodeId,
    mut inputs: LayoutInput,
) -> LayoutInput {
    // Inline tables use the table width algorithm (CSS 2 §17.5.2), including
    // its content minimum. Let that algorithm select the used dimensions.
    if tree.get_oof_item_style(node).is_table() {
        return inputs;
    }
    let constraints = box_constraints::resolve(tree, node, inputs.parent_size, inputs.parent_size.width);
    inputs.known_dimensions = inputs.known_dimensions.or(constraints.known());
    inputs.known_dimensions_are_definite = inputs.known_dimensions.map(|v| v.is_some());
    if inputs.known_dimensions.width.is_none() {
        let width = tree.compute_child_layout(node, LayoutInput {
            run_mode: RunMode::ComputeSize,
            axis: RequestedAxis::Horizontal,
            ..inputs
        }).size.width;
        inputs.known_dimensions.width = Some(width);
        // Fit-content against definite available space is definite (Sizing 3).
        inputs.known_dimensions_are_definite.width = inputs.available_space.width.is_definite();
    }
    if inputs.known_dimensions.height.is_none() {
        let content_height = tree.compute_child_layout(node, LayoutInput {
            run_mode: RunMode::ComputeSize,
            axis: RequestedAxis::Vertical,
            ..inputs
        }).size.height;
        let style = tree.get_core_container_style(node);
        let scroll = style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container();
        let ratio_height = constraints.aspect_ratio.filter(|ratio| *ratio > 0.0).map(|ratio| {
            (inputs.known_dimensions.width.unwrap_or(0.0) - constraints.adjustment.width).max(0.0) / ratio
                + constraints.adjustment.height
        });
        let height = ratio_height.map_or(content_height, |height| {
            if scroll { height } else { height.max(content_height) }
        });
        inputs.known_dimensions.height = Some(height
            .maybe_clamp(constraints.minimum.height, constraints.maximum.height)
            .max(constraints.padding_border.height));
        inputs.known_dimensions_are_definite.height = inputs.known_dimensions_are_definite.width
            && ratio_height.is_some_and(|height| scroll || height >= content_height);
    }
    inputs
}
