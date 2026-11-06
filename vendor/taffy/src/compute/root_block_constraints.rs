//! Root blocks fill their logical inline axis, with physical sizes and ratio constraints preserved.
use crate::tree::LayoutPartialTreeExt;
use crate::{
    AvailableSpace, CoreStyle, LayoutPartialTree, MaybeMath, NodeId, ResolveOrZero,
    Size,
};

/// KaminIDE patch: Resolve the block root's definite dimensions and constraints.
pub(super) fn known(
    tree: &impl LayoutPartialTree,
    root: NodeId,
    available_space: Size<AvailableSpace>,
) -> Size<Option<f32>> {
    let mut known_dimensions = Size::NONE;

    {
        let parent_size = available_space.into_options();
        let style = tree.get_core_container_style(root);
        let vertical = style.block_flow().is_some_and(|flow| flow.vertical);
        let inline_basis = if vertical {
            parent_size.height
        } else {
            parent_size.width
        };

        if style.is_block() || style.is_flow_root() {
            // Pull these out earlier to avoid borrowing issues
            let margin = style
                .margin()
                .resolve_or_zero(inline_basis, |val, basis| tree.calc(val, basis));
            let constraints = super::common::box_constraints::resolve(tree, root, parent_size, inline_basis);

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
                .or(constraints.known())
                .or(available_space_based_size)
                .maybe_max(constraints.padding_border);

            known_dimensions = styled_based_known_dimensions;
        }
    }

    known_dimensions
}
