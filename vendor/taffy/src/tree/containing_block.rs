//! Operations shared by containing blocks and the out-of-flow positioning pass.
use super::LayoutPartialTree;
use crate::style::OofItemStyle;
use crate::tree::{DetailedLayoutInfo, NodeId};

/// Extends [`LayoutPartialTree`] with the operations needed by the out-of-flow positioning pass
/// ([`compute_oof_layout`](crate::compute_oof_layout)), which lays out the absolute/fixed boxes
/// for which a node acts as the containing block.
///
/// This trait is required by [`compute_oof_layout`](crate::compute_oof_layout) and
/// [`compute_root_layout`](crate::compute_root_layout).
pub trait LayoutContainingBlock: LayoutPartialTree {
    /// Consume changes to intrinsic sizes caused by gutters during descendant layout.
    /// Positioning must repeat its size selection before committing a forced final size.
    fn take_intrinsic_size_change(&mut self, node_id: NodeId) -> bool {
        let _ = node_id;
        false
    }

    /// The style type representing the styles of an out-of-flow (absolute/fixed) box being
    /// positioned by its containing block
    type OofItemStyle<'a>: OofItemStyle<CustomIdent = Self::CustomIdent>
    where
        Self: 'a;

    /// Get the style of an out-of-flow box being positioned by its containing block
    ///
    /// Interpreted in the containing block's axes (see [`LayoutPartialTree::get_core_container_style`]).
    fn get_oof_item_style(&self, node_id: NodeId) -> Self::OofItemStyle<'_>;

    /// Clear the list of out-of-flow (absolute/fixed) boxes whose containing block is `node_id`.
    ///
    /// This is called (exactly once) for each node laid out with `RunMode::PerformLayout`, before
    /// the boxes it lays out are recorded with [`add_hoisted_children`](Self::add_hoisted_children),
    /// so that lists recorded by previous layout runs do not persist.
    fn clear_hoisted_children(&mut self, node_id: NodeId);

    /// Append to the list of out-of-flow boxes whose containing block is `node_id`.
    ///
    /// This is called by each containing block for the boxes it lays out, and by
    /// [`compute_root_layout`](crate::compute_root_layout) to record boxes (e.g. `position: fixed`
    /// boxes) which are positioned by the final root positioning pass, which runs after the root
    /// node's own layout algorithm has already recorded its list. The recorded lists are consumed
    /// by [`round_layout`](crate::round_layout) (out-of-flow boxes are rounded via their
    /// containing block rather than via their parent), and are also useful for consumers
    /// implementing paint/hit-testing traversals.
    fn add_hoisted_children(&mut self, node_id: NodeId, hoisted: &[NodeId]);

    /// Read back the detailed layout information most recently recorded for `node_id`
    /// (as stored by [`LayoutGridContainer::set_detailed_grid_info`](crate::tree::LayoutGridContainer::set_detailed_grid_info)).
    ///
    /// This is used by the out-of-flow positioning pass to resolve the grid area of absolutely
    /// positioned boxes whose containing block is a grid container. Implementing this method is
    /// optional: the default implementation returns [`DetailedLayoutInfo::None`], in which case
    /// such boxes are positioned relative to the grid container's padding box instead of their
    /// grid area.
    #[inline(always)]
    fn get_detailed_layout_info(&self, node_id: NodeId) -> &DetailedLayoutInfo<Self::CustomIdent> {
        let _ = node_id;
        &DetailedLayoutInfo::None
    }

    /// KaminIDE patch: how far the inset-resolution area of out-of-flow `node_id` moves in on
    /// each side because the viewport decided other bars than the box was built for
    /// (`ScrollbarLayout::root_gutter`). `None` leaves the area as it is.
    #[inline(always)]
    fn kamin_oof_area_shift(&self, node_id: NodeId) -> Option<crate::geometry::Rect<f32>> {
        let _ = node_id;
        None
    }

    /// KaminIDE patch: whether out-of-flow `node_id` is a scroll range proxy, which adds
    /// nothing to its containing block's scrollable overflow (`ScrollbarLayout::proxy`).
    #[inline(always)]
    fn kamin_oof_is_proxy(&self, node_id: NodeId) -> bool {
        let _ = node_id;
        false
    }
}
