//! Gutter changes invalidate intrinsic sizes before an ancestor commits its result.
//! As Blink's BlockNode retry, freeze appeared bars for the rest of the layout transaction
//! while the box keeps the size they appeared at.
use super::*;
use crate::geometry::{Point, Size};

/// A native layout decision, including the constraints that distinguish speculative probes.
#[derive(Clone, Debug)]
pub struct LayoutDependencyEvent {
    /// Stable event code.
    pub kind: &'static str,
    /// The box whose gutter changed or whose layout must repeat.
    pub node: NodeId,
    /// Constraints of this invocation.
    pub inputs: LayoutInput,
    /// Result before the dependency retry.
    pub output: LayoutOutput,
    /// Whether this box changed its own gutter.
    pub own_gutter: bool,
    /// Whether a descendant invalidated this box.
    pub descendant_gutter: bool,
}

// Keep diagnostic event temporaries out of every recursive layout stack frame.
#[cold]
#[inline(never)]
fn observe_dependency_retry(
    observe: fn(LayoutDependencyEvent),
    node: NodeId,
    inputs: &LayoutInput,
    output: &LayoutOutput,
    own_gutter: bool,
    descendant_gutter: bool,
) {
    observe(LayoutDependencyEvent {
        kind: "layout.dependency_retry",
        node,
        inputs: *inputs,
        output: output.clone(),
        own_gutter,
        descendant_gutter,
    });
}

/// Global configuration values for a TaffyTree instance
#[derive(Debug, Clone, Copy)]
pub(crate) struct TaffyConfig {
    /// Whether to round layout values
    pub(crate) use_rounding: bool,
    /// Optional observer of scrollbar decisions and the ancestor passes they invalidate.
    pub(super) layout_observer: Option<fn(LayoutDependencyEvent)>,
}

impl Default for TaffyConfig {
    fn default() -> Self {
        Self { use_rounding: true, layout_observer: None }
    }
}

impl<NodeContext> TaffyTree<NodeContext> {
    /// Observe native dependency retries. With no observer, no event is constructed.
    pub fn set_layout_observer(&mut self, observer: Option<fn(LayoutDependencyEvent)>) {
        self.config.layout_observer = observer;
    }
}

/// An active ancestor must repeat its layout with the settled descendant gutter.
const DIRTY: u8 = 1;
/// Intrinsic size selection must run again, independently of the local layout retry.
pub(super) const SIZE_DIRTY: u8 = 16;
/// The view owns this node in its transaction cleanup list.
const TOUCHED: u8 = 8;
/// A horizontal bar appeared and stays while the box keeps the width it appeared at.
const FROZEN_X: u8 = 2;
/// A vertical bar appeared and stays while the box keeps the height it appeared at.
const FROZEN_Y: u8 = 4;

/// The nodes a layout transaction changed the scrollbar state of, and the border-box
/// size each one's bars appeared at.
#[derive(Default)]
pub(crate) struct ScrollbarTouched {
    /// Every node whose scrollbar state this transaction set, cleared when the view drops.
    nodes: Vec<NodeId>,
    /// Only boxes whose bars changed; looked up by the box's own settling only.
    frozen_at: Vec<(NodeId, Size<f32>)>,
}

impl<NodeContext, MeasureFunction> TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
{
    /// Only the ancestor chain loses intrinsic measurements; unrelated subtrees stay cached.
    pub(super) fn kamin_gutter_changed(&mut self, node_id: NodeId, bars: Point<bool>, at: Size<f32>) {
        self.touch_scrollbar_state(node_id);
        let frozen = &mut self.scrollbar_touched.frozen_at;
        match frozen.iter_mut().find(|(id, _)| *id == node_id) {
            Some((_, size)) => *size = at,
            None => frozen.push((node_id, at)),
        }
        // A bar taken back is no longer frozen: it was not needed at this size.
        let state = &mut self.taffy.nodes[node_id.into()].scrollbar_state;
        *state = (*state & !(FROZEN_X | FROZEN_Y))
            | SIZE_DIRTY
            | if bars.x { FROZEN_X } else { 0 }
            | if bars.y { FROZEN_Y } else { 0 };
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
            self.scrollbar_touched.nodes.push(node_id);
            self.taffy.nodes[node_id.into()].scrollbar_state = TOUCHED;
        }
    }

    /// Bars that appeared in any local retry of this transaction and still hold at the
    /// box's `size`. A horizontal bar answers horizontal overflow, so it holds while the
    /// width it was decided at does; a vertical bar while the height does. A box whose
    /// inline size follows its own gutter (shrink-to-fit, CSS 2.1 §10.3.5 and §10.3.7)
    /// is laid out once more at the width its parent gives it with the new bar: the other
    /// bar is decided again there, as Blink decides it after `SetIntrinsicLogicalWidthsDirty`
    /// (`PaintLayerScrollableArea::UpdateAfterLayout`), so the bars do not depend on the
    /// guess the box was built with.
    pub(super) fn kamin_frozen_scrollbars(&self, node_id: NodeId, size: Size<f32>) -> Point<bool> {
        let flags = self.taffy.nodes[node_id.into()].scrollbar_state;
        if flags & (FROZEN_X | FROZEN_Y) == 0 {
            return Point { x: false, y: false };
        }
        let at = self.scrollbar_touched.frozen_at.iter().find(|(id, _)| *id == node_id).map(|(_, at)| *at);
        Point {
            x: flags & FROZEN_X != 0 && at.is_some_and(|at| at.width == size.width),
            y: flags & FROZEN_Y != 0 && at.is_some_and(|at| at.height == size.height),
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
            if let Some(observe) = self.taffy.config.layout_observer {
                observe_dependency_retry(
                    observe,
                    node_id,
                    &inputs,
                    &output,
                    own_bars_changed,
                    self.taffy.nodes[node_id.into()].scrollbar_state & DIRTY != 0,
                );
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
        for id in &self.scrollbar_touched.nodes {
            self.taffy.nodes[(*id).into()].scrollbar_state = 0;
        }
    }
}
