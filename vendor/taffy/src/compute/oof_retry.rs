//! Repeat only an out-of-flow box whose descendant gutters changed its intrinsic size.
use super::*;

/// Size and position a single out-of-flow box against its containing block, and store its layout.
///
/// - `area_size`/`area_offset` describe the inset-resolution area of the containing block
///   (border box minus borders and scrollbar gutters), relative to the containing block's border box.
/// - `container_overflow` is the `overflow` style of the containing block, which determines
///   whether overflowing boxes may be aligned into its scrollable overflow area.
/// - `grid_owner` is the node whose detailed layout info is consulted for grid placement: if it is
///   a grid container, the box is positioned relative to the grid area determined by its
///   grid-placement properties rather than the passed area.
/// - `before_layout` is called with the inputs of the final layout pass just before it runs, which
///   allows the caller to inspect the layout cache for the box.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn layout_oof_box<Tree: LayoutContainingBlock>(
    tree: &mut Tree,
    candidate: OofCandidate,
    area_size: Size<f32>,
    area_offset: Point<f32>,
    direction: Direction,
    container_overflow: Point<Overflow>,
    #[cfg(feature = "grid")] grid_owner: Option<NodeId>,
    mut before_layout: impl FnMut(&mut Tree, &LayoutInput),
) -> OofBoxLayout {
    tree.take_intrinsic_size_change(candidate.node);
    loop {
        let result = super::layout_oof_box_once(
            tree,
            candidate,
            area_size,
            area_offset,
            direction,
            container_overflow,
            #[cfg(feature = "grid")]
            grid_owner,
            &mut before_layout,
        );
        if !tree.take_intrinsic_size_change(candidate.node) {
            return result;
        }
        // The invalidated intrinsic cache now measures the settled gutters. An
        // explicit author size is resolved again unchanged; a fit-content size can grow.
    }
}
