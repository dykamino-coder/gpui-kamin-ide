//! Gutter changes invalidate intrinsic sizes before an ancestor commits its result.
//! As Blink's BlockNode retry, freeze appeared bars for the whole layout transaction.
use super::*;
use crate::geometry::Point;

/// An active ancestor must repeat its layout with the settled descendant gutter.
const DIRTY: u8 = 1;
/// Intrinsic size selection must run again, independently of the local layout retry.
pub(super) const SIZE_DIRTY: u8 = 16;
/// The view owns this node in its transaction cleanup list.
const TOUCHED: u8 = 8;
/// A horizontal bar appeared and stays until this layout transaction ends.
const FROZEN_X: u8 = 2;
/// A vertical bar appeared and stays until this layout transaction ends.
const FROZEN_Y: u8 = 4;

impl<NodeContext, MeasureFunction> TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
{
    /// Only the ancestor chain loses intrinsic measurements; unrelated subtrees stay cached.
    pub(super) fn kamin_gutter_changed(&mut self, node_id: NodeId, bars: Point<bool>) {
        self.touch_scrollbar_state(node_id);
        self.taffy.nodes[node_id.into()].scrollbar_state |=
            SIZE_DIRTY | if bars.x { FROZEN_X } else { 0 } | if bars.y { FROZEN_Y } else { 0 };
        let mut parent = self.taffy.parents[node_id.into()];
        while let Some(id) = parent {
            if self.taffy.nodes[id.into()].scrollbar_state & (DIRTY | SIZE_DIRTY)
                == DIRTY | SIZE_DIRTY
            {
                break;
            }
            self.touch_scrollbar_state(id);
            let node = &mut self.taffy.nodes[id.into()];
            node.scrollbar_state |= DIRTY | SIZE_DIRTY;
            node.cache.clear();
            #[cfg(feature = "grid")]
            self.grid_memos.0.retain(|(grid, _)| *grid != id);
            parent = self.taffy.parents[id.into()];
        }
    }

    /// Register a changed node once so dropping the view clears transient state.
    fn touch_scrollbar_state(&mut self, node_id: NodeId) {
        if self.taffy.nodes[node_id.into()].scrollbar_state == 0 {
            self.scrollbar_touched.push(node_id);
            self.taffy.nodes[node_id.into()].scrollbar_state = TOUCHED;
        }
    }

    /// Bars that appeared in any local retry of this transaction.
    pub(super) fn kamin_frozen_scrollbars(&self, node_id: NodeId) -> Point<bool> {
        let flags = self.taffy.nodes[node_id.into()].scrollbar_state;
        Point {
            x: flags & FROZEN_X != 0,
            y: flags & FROZEN_Y != 0,
        }
    }

    /// Repeat an affected active ancestor locally, using the children's now-stable gutters.
    /// Cached descendants make this a dependency retry rather than a document restart.
    #[inline(always)]
    pub(super) fn compute_with_scrollbar_dependencies(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        #[cfg(feature = "block_layout")] mut block_ctx: Option<&mut BlockContext<'_>>,
    ) -> LayoutOutput {
        let mut settling = kamin_scrollbars::Settling::default();
        loop {
            self.taffy.nodes[node_id.into()].scrollbar_state &= !DIRTY;
            self.kamin_publish_root_gutter(node_id);
            let display_mode = self.taffy.nodes[node_id.into()].style.display;
            let has_children = self.child_count(node_id) > 0;
            debug_log!(display_mode);
            debug_log_node!(inputs);
            let output = match (display_mode, has_children) {
                #[cfg(feature = "block_layout")]
                (Display::Block, true) => {
                    compute_block_layout(self, node_id, inputs, block_ctx.as_deref_mut())
                }
                #[cfg(feature = "block_layout")]
                (Display::FlowRoot, true) => compute_block_layout(self, node_id, inputs, None),
                _ => self.compute_other_layout(node_id, inputs, display_mode, has_children),
            };
            let output = self.finish_child_layout(node_id, inputs.run_mode, output);
            let own_bars_changed =
                self.kamin_settle_scrollbars(node_id, inputs, &output, &mut settling);
            if !own_bars_changed && self.taffy.nodes[node_id.into()].scrollbar_state & DIRTY == 0 {
                return output;
            }
            #[cfg(feature = "grid")]
            self.grid_memos.0.retain(|(grid, _)| *grid != node_id);
        }
    }
}

impl<NodeContext, MeasureFunction> Drop for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
{
    fn drop(&mut self) {
        for id in &self.scrollbar_touched {
            self.taffy.nodes[(*id).into()].scrollbar_state = 0;
        }
    }
}
