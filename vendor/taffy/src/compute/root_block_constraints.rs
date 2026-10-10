//! Root blocks fill their logical inline axis, with physical sizes and ratio constraints preserved.
use crate::tree::LayoutPartialTreeExt;
use crate::{
    AvailableSpace, CoreStyle, LayoutPartialTree, MaybeMath, MaybeResolve, NodeId, ResolveOrZero,
    Size,
};

pub(super) fn known(
    tree: &impl LayoutPartialTree,
    root: NodeId,
    available_space: Size<AvailableSpace>,
) -> Size<Option<f32>> {
    let mut known_dimensions = Size::NONE;

    {
        use crate::BoxSizing;

        let parent_size = available_space.into_options();
        let style = tree.get_core_container_style(root);
        let vertical = style.block_flow().is_some_and(|flow| flow.vertical);
        let inline_basis = if vertical {
            parent_size.height
        } else {
            parent_size.width
        };

        if style.is_block() {
            // Pull these out earlier to avoid borrowing issues
            let aspect_ratio = style.aspect_ratio();
            let margin = style
                .margin()
                .resolve_or_zero(inline_basis, |val, basis| tree.calc(val, basis));
            let padding = style
                .padding()
                .resolve_or_zero(inline_basis, |val, basis| tree.calc(val, basis));
            let border = style
                .border()
                .resolve_or_zero(inline_basis, |val, basis| tree.calc(val, basis));
            let padding_border_size = (padding + border).sum_axes();
            let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
                padding_border_size
            } else {
                Size::ZERO
            };

            let preferred = style
                .size()
                .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis));
            let (min_size, max_size) = super::common::aspect_ratio_constraints::transfer(
                preferred,
                style
                    .min_size()
                    .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis)),
                style
                    .max_size()
                    .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis)),
                aspect_ratio,
            );
            let min_size = min_size.maybe_add(box_sizing_adjustment);
            let max_size = max_size.maybe_add(box_sizing_adjustment);
            let clamped_style_size = preferred
                .maybe_apply_aspect_ratio(aspect_ratio)
                .maybe_add(box_sizing_adjustment)
                .maybe_clamp(min_size, max_size);

            // If both min and max in a given axis are set and max <= min then this determines the size in that axis
            let min_max_definite_size = min_size.zip_map(max_size, |min, max| match (min, max) {
                (Some(min), Some(max)) if max <= min => Some(min),
                _ => None,
            });

            // Block nodes automatically stretch fit their width to fit available space if available space is definite
            let available_space_based_size = Size {
                width: if vertical {
                    None
                } else {
                    parent_size.width.maybe_sub(margin.horizontal_axis_sum())
                },
                height: if vertical {
                    parent_size.height.maybe_sub(margin.vertical_axis_sum())
                } else {
                    None
                },
            };

            let styled_based_known_dimensions = known_dimensions
                .or(min_max_definite_size)
                .or(clamped_style_size)
                .or(available_space_based_size)
                .maybe_max(padding_border_size);

            known_dimensions = styled_based_known_dimensions;
        }
    }

    known_dimensions
}
