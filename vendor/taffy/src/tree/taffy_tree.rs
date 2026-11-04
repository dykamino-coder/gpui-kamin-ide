//! Contains [TaffyTree](crate::tree::TaffyTree): the default implementation of [LayoutTree](crate::tree::LayoutTree), and the error type for Taffy.
#[cfg(not(feature = "std"))]
use slotmap::SecondaryMap;
#[cfg(feature = "std")]
use slotmap::SparseSecondaryMap as SecondaryMap;
use slotmap::{DefaultKey, SlotMap};

/// KaminIDE patch: a node style, shared by the nodes that have the same one.
///
/// A `Style` is about 1 KiB and the boxes of one frame tree mostly repeat a few styles, so a
/// node holds a reference and the host may give equal styles one allocation (gpui
/// `TaffyLayoutEngine::request_layout`). Layout only reads styles; `set_style` replaces it.
#[cfg(feature = "std")]
pub type SharedStyle = std::rc::Rc<Style>;
/// KaminIDE patch: a node style, shared by the nodes that have the same one.
#[cfg(not(feature = "std"))]
pub type SharedStyle = alloc::rc::Rc<Style>;

use super::kamin_calc::calc_value;
#[path = "calc_size.rs"]
mod calc_size;
#[path = "computed_output.rs"]
mod computed_output;
#[path = "node_lifetime.rs"]
mod node_lifetime;
#[cfg(feature = "block_layout")]
use crate::block::BlockContext;
use crate::geometry::Size;
use crate::style::{AvailableSpace, Display, Style};
use crate::sys::DefaultCheapStr;
use crate::tree::{
    Cache, ClearState, Layout, LayoutContainingBlock, LayoutInput, LayoutOutput, LayoutPartialTree,
    NodeId, PrintTree, RoundTree, RunMode, TraversePartialTree, TraverseTree,
};
use crate::util::debug::{debug_log, debug_log_node};
use crate::util::sys::{new_const_children_vec, new_vec_with_capacity, Box, ChildrenVec, Vec};

use crate::compute::{
    compute_cached_layout_async, compute_leaf_layout, compute_oof_layout_async,
    compute_root_layout_async, round_layout,
};
use crate::CacheTree;

#[cfg(feature = "block_layout")]
use crate::{compute::compute_block_layout_async, LayoutBlockContainer};
#[cfg(feature = "flexbox")]
use crate::{compute::compute_flexbox_layout_async, LayoutFlexboxContainer};
#[cfg(feature = "grid")]
use crate::{compute::compute_grid_layout_async, LayoutGridContainer};

#[cfg(feature = "grid")]
use crate::compute::grid::DetailedGridInfo;
use crate::tree::layout::DetailedLayoutInfo;

/// The error Taffy generates on invalid operations
pub type TaffyResult<T> = Result<T, TaffyError>;

/// An error that occurs while trying to access or modify a node's children by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaffyError {
    /// The parent node does not have a child at `child_index`. It only has `child_count` children
    ChildIndexOutOfBounds {
        /// The parent node whose child was being looked up
        parent: NodeId,
        /// The index that was looked up
        child_index: usize,
        /// The total number of children the parent has
        child_count: usize,
    },
    /// The parent node was not found in the [`TaffyTree`](crate::TaffyTree) instance.
    InvalidParentNode(NodeId),
    /// The child node was not found in the [`TaffyTree`](crate::TaffyTree) instance.
    InvalidChildNode(NodeId),
    /// The supplied node was not found in the [`TaffyTree`](crate::TaffyTree) instance.
    InvalidInputNode(NodeId),
}

impl core::fmt::Display for TaffyError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            TaffyError::ChildIndexOutOfBounds {
                parent,
                child_index,
                child_count,
            } => {
                write!(f, "Index (is {child_index}) should be < child_count ({child_count}) for parent node {parent:?}")
            }
            TaffyError::InvalidParentNode(parent) => {
                write!(f, "Parent Node {parent:?} is not in the TaffyTree instance")
            }
            TaffyError::InvalidChildNode(child) => {
                write!(f, "Child Node {child:?} is not in the TaffyTree instance")
            }
            TaffyError::InvalidInputNode(node) => {
                write!(f, "Supplied Node {node:?} is not in the TaffyTree instance")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TaffyError {}

/// Global configuration values for a TaffyTree instance
#[derive(Debug, Clone, Copy)]
pub(crate) struct TaffyConfig {
    /// Whether to round layout values
    pub(crate) use_rounding: bool,
}

impl Default for TaffyConfig {
    fn default() -> Self {
        Self { use_rounding: true }
    }
}

/// Layout information for a given [`Node`](crate::node::Node)
///
/// Stored in a [`TaffyTree`].
#[derive(Debug, Clone, PartialEq)]
struct NodeData {
    /// The layout strategy used by this node
    pub(crate) style: SharedStyle,

    /// The always unrounded results of the layout computation. We must store this separately from the rounded
    /// layout to avoid errors from rounding already-rounded values. See <https://github.com/DioxusLabs/taffy/issues/501>.
    pub(crate) unrounded_layout: Layout,

    /// The final results of the layout computation.
    /// These may be rounded or unrounded depending on what the `use_rounding` config setting is set to.
    ///
    /// KaminIDE patch: written only by rounding and allocated then; a tree that does not round
    /// reads `unrounded_layout` and no longer carries a second `Layout` in every node.
    pub(crate) final_layout: Option<Box<Layout>>,

    /// Whether the node has context data associated with it or not
    pub(crate) has_context: bool,

    /// Out-of-flow (absolute/fixed) boxes whose containing block is this node,
    /// as recorded by the out-of-flow positioning pass
    pub(crate) hoisted_children: ChildrenVec<NodeId>,

    /// The cached results of the layout computation
    pub(crate) cache: Cache,

    /// The computation result from layout algorithm
    pub(crate) detailed_layout_info: DetailedLayoutInfo,
    /// Tracks published by the parent subgrid layout, outside the native cache key.
    #[cfg(feature = "grid")]
    /// KaminIDE patch: boxed: only subgrid items carry tracks, inline they were 160 bytes of
    /// every node.
    pub(crate) subgrid_tracks: Option<Box<crate::style::SubgridTracks>>,
    /// Parent-height basis used by an anonymous row's percentage-height children.
    pub(crate) pct_parent_h: Option<Option<f32>>,
    /// KaminIDE patch: y offset of this node's children inside its box (see
    /// `LayoutGridContainer::set_content_shift`).
    pub(crate) content_shift: f32,
    /// KaminIDE patch: fragmentation inputs, allocated by the first non-default setter
    /// (see [`NodeFragmentation`]).
    pub(crate) fragmentation: Option<Box<NodeFragmentation>>,
}

/// KaminIDE patch: the fragmentation inputs of a node (css-break-3, css-flexbox-1 §10,
/// css-grid-2 §12). Only boxes inside a fragmentation context set them; inline they were about
/// 90 bytes of every node of every frame tree. A node without the block reads the defaults.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeFragmentation {
    /// KaminIDE patch: consumed fragmentainer space after physical row flex lines.
    #[cfg(feature = "flexbox")]
    flex_line_growth: Vec<f32>,
    /// KaminIDE patch: consumed block space before a column flex item.
    #[cfg(feature = "flexbox")]
    flex_item_offset: f32,
    /// KaminIDE patch: consumed border-box cross extent in a row continuation.
    #[cfg(feature = "flexbox")]
    flex_item_consumed: f32,
    /// KaminIDE patch: flex line of this item fixed before fragmentation.
    #[cfg(feature = "flexbox")]
    flex_item_line: Option<usize>,
    /// KaminIDE patch: the flex container is laid out in a fragmentation context.
    #[cfg(feature = "flexbox")]
    flex_fragmentation: bool,
    /// KaminIDE patch: physical block axis and block-start direction of fragmentation.
    #[cfg(feature = "flexbox")]
    flex_fragmentation_axis: (crate::geometry::AbsoluteAxis, bool),
    /// KaminIDE patch: fragmentation cannot expand a monolithic item.
    #[cfg(feature = "flexbox")]
    fragmentation_monolithic: bool,
    /// KaminIDE patch: block space inserted after the tracks of one physical grid axis.
    #[cfg(feature = "grid")]
    grid_track_growth: (crate::geometry::AbsoluteAxis, Vec<f32>),
}

impl NodeFragmentation {
    /// The inputs of a node outside any fragmentation context.
    const NONE: Self = Self {
        #[cfg(feature = "flexbox")]
        flex_line_growth: Vec::new(),
        #[cfg(feature = "flexbox")]
        flex_item_offset: 0.0,
        #[cfg(feature = "flexbox")]
        flex_item_consumed: 0.0,
        #[cfg(feature = "flexbox")]
        flex_item_line: None,
        #[cfg(feature = "flexbox")]
        flex_fragmentation: false,
        #[cfg(feature = "flexbox")]
        flex_fragmentation_axis: (crate::geometry::AbsoluteAxis::Vertical, false),
        #[cfg(feature = "flexbox")]
        fragmentation_monolithic: false,
        #[cfg(feature = "grid")]
        grid_track_growth: (crate::geometry::AbsoluteAxis::Vertical, Vec::new()),
    };
}

/// KaminIDE patch: the fragmentation inputs of a node that no setter has changed.
static NO_FRAGMENTATION: NodeFragmentation = NodeFragmentation::NONE;

/// KaminIDE patch: the final layout of a node that rounding has not written yet.
static NO_FINAL_LAYOUT: Layout = Layout::new();

impl NodeData {
    /// Create the data for a new node
    #[must_use]
    pub const fn new(style: SharedStyle) -> Self {
        Self {
            style,
            cache: Cache::new(),
            unrounded_layout: Layout::new(),
            final_layout: None,
            has_context: false,
            hoisted_children: new_const_children_vec(),
            detailed_layout_info: DetailedLayoutInfo::None,
            #[cfg(feature = "grid")]
            subgrid_tracks: None,
            pct_parent_h: None,
            content_shift: 0.0,
            fragmentation: None,
        }
    }

    /// Marks a node and all of its ancestors as requiring relayout
    ///
    /// This clears any cached data and signals that the data must be recomputed.
    /// If the node was already marked as dirty, returns true
    #[inline]
    pub fn mark_dirty(&mut self) -> ClearState {
        self.cache.clear()
    }

    /// KaminIDE patch: the fragmentation inputs, defaults when none was set.
    fn fragmentation(&self) -> &NodeFragmentation {
        self.fragmentation.as_deref().unwrap_or(&NO_FRAGMENTATION)
    }

    /// KaminIDE patch: the fragmentation inputs for writing; allocates them on first use.
    fn fragmentation_mut(&mut self) -> &mut NodeFragmentation {
        self.fragmentation
            .get_or_insert_with(|| Box::new(NodeFragmentation::NONE))
    }
}

/// An entire tree of UI nodes. The entry point to Taffy's high-level API.
///
/// Allows you to build a tree of UI nodes, run Taffy's layout algorithms over that tree, and then access the resultant layout.]
#[derive(Debug, Clone)]
pub struct TaffyTree<NodeContext = ()> {
    /// The [`NodeData`] for each node stored in this tree
    nodes: SlotMap<DefaultKey, NodeData>,

    /// Functions/closures that compute the intrinsic size of leaf nodes
    node_context_data: SecondaryMap<DefaultKey, NodeContext>,

    /// The children of each node
    ///
    /// The indexes in the outer vector correspond to the position of the parent [`NodeData`]
    children: SlotMap<DefaultKey, ChildrenVec<NodeId>>,

    /// The parents of each node
    ///
    /// The indexes in the outer vector correspond to the position of the child [`NodeData`]
    parents: SlotMap<DefaultKey, Option<NodeId>>,

    /// Layout mode configuration
    config: TaffyConfig,

    /// KaminIDE patch: `calc()` pairs referenced by the styles of this tree's nodes
    /// (`kamin_calc`); cleared with the nodes.
    #[cfg(feature = "std")]
    calc: super::kamin_calc::SharedRegistry,
}

impl Default for TaffyTree {
    fn default() -> TaffyTree<()> {
        TaffyTree::new()
    }
}

#[cfg(feature = "flexbox")]
impl<NodeContext> TaffyTree<NodeContext> {
    /// KaminIDE patch: fragmentation follows the writing-mode block axis (CSS Writing Modes 3 §6).
    /// Reversal affects consumed-space placement, never flex sizing or line membership.
    pub fn set_flex_fragmentation_axis(
        &mut self,
        node: NodeId,
        axis: crate::geometry::AbsoluteAxis,
        reversed: bool,
    ) -> TaffyResult<()> {
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_fragmentation_axis != (axis, reversed) {
            data.fragmentation_mut().flex_fragmentation_axis = (axis, reversed);
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: content extent already consumed by a row flex item.
    /// The item retains its main size and full contents; only its remaining
    /// cross contribution and final cross position change (CSS Flexbox 1 §10).
    pub fn set_flex_item_fragmentation_consumed(
        &mut self,
        node: NodeId,
        consumed: f32,
    ) -> TaffyResult<()> {
        if !consumed.is_finite() || consumed < 0.0 {
            return Err(TaffyError::InvalidInputNode(node));
        }
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_item_consumed != consumed {
            data.fragmentation_mut().flex_item_consumed = consumed;
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: consumed block space before a column flex item.
    /// Applied after flex sizing, so a break cannot redistribute flexible lengths.
    pub fn set_flex_item_fragmentation_offset(
        &mut self,
        node: NodeId,
        offset: f32,
    ) -> TaffyResult<()> {
        if !offset.is_finite() {
            return Err(TaffyError::InvalidInputNode(node));
        }
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_item_offset != offset {
            data.fragmentation_mut().flex_item_offset = offset;
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: the flex line of an item, fixed before fragmentation.
    /// Line breaking precedes fragmentation (css-flexbox-1 §10): growth imposed
    /// by fragmentation inside an item must not move it to another line.
    pub fn set_flex_item_fragmentation_line(
        &mut self,
        node: NodeId,
        line: Option<usize>,
    ) -> TaffyResult<()> {
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_item_line != line {
            data.fragmentation_mut().flex_item_line = line;
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: caller-owned monolithic classification, independent of style.
    pub fn set_fragmentation_monolithic(&mut self, node: NodeId, on: bool) -> TaffyResult<()> {
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().fragmentation_monolithic != on {
            data.fragmentation_mut().fragmentation_monolithic = on;
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: the flex container is laid out in a fragmentation context.
    pub fn set_flex_fragmentation(&mut self, node: NodeId, on: bool) -> TaffyResult<()> {
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_fragmentation != on {
            data.fragmentation_mut().flex_fragmentation = on;
            self.mark_dirty(node)?;
        }
        Ok(())
    }

    /// KaminIDE patch: consumed space after row flex lines (css-flexbox-1 §10).
    /// A negative value suppresses that much of the gap after the line at a break.
    /// This constraint is outside the style/cache key; changing it invalidates layout.
    pub fn set_flex_line_fragmentation_growth(
        &mut self,
        node: NodeId,
        growth: Vec<f32>,
    ) -> TaffyResult<()> {
        if growth.iter().any(|v| !v.is_finite()) {
            return Err(TaffyError::InvalidInputNode(node));
        }
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        if data.fragmentation().flex_line_growth != growth {
            data.fragmentation_mut().flex_line_growth = growth;
            self.mark_dirty(node)?;
        }
        Ok(())
    }
}

#[cfg(feature = "grid")]
impl<NodeContext> TaffyTree<NodeContext> {
    /// KaminIDE patch: block space inserted after each track of the physical `axis` of a grid
    /// container (css-grid-2 §12), indexed like `DetailedGridTracksInfo::positions`. This
    /// constraint is outside the style/cache key; changing it invalidates layout.
    pub fn set_grid_fragmentation_growth(
        &mut self,
        node: NodeId,
        axis: crate::geometry::AbsoluteAxis,
        growth: Vec<f32>,
    ) -> TaffyResult<()> {
        if growth.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(TaffyError::InvalidInputNode(node));
        }
        let data = self
            .nodes
            .get_mut(node.into())
            .ok_or(TaffyError::InvalidInputNode(node))?;
        let growth = (axis, growth);
        if data.fragmentation().grid_track_growth != growth {
            data.fragmentation_mut().grid_track_growth = growth;
            self.mark_dirty(node)?;
        }
        Ok(())
    }
}

/// Iterator that wraps a slice of nodes, lazily converting them to u64
pub struct TaffyTreeChildIter<'a>(core::slice::Iter<'a, NodeId>);
impl Iterator for TaffyTreeChildIter<'_> {
    type Item = NodeId;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().copied()
    }
}

// TraversePartialTree impl for TaffyTree
impl<NodeContext> TraversePartialTree for TaffyTree<NodeContext> {
    type ChildIter<'a>
        = TaffyTreeChildIter<'a>
    where
        Self: 'a;

    #[inline(always)]
    fn child_ids(&self, parent_node_id: NodeId) -> Self::ChildIter<'_> {
        TaffyTreeChildIter(self.children[parent_node_id.into()].iter())
    }

    #[inline(always)]
    fn child_count(&self, parent_node_id: NodeId) -> usize {
        self.children[parent_node_id.into()].len()
    }

    #[inline(always)]
    fn get_child_id(&self, parent_node_id: NodeId, id: usize) -> NodeId {
        self.children[parent_node_id.into()][id]
    }
}

// TraverseTree impl for TaffyTree
impl<NodeContext> TraverseTree for TaffyTree<NodeContext> {}

// CacheTree impl for TaffyTree
impl<NodeContext> CacheTree for TaffyTree<NodeContext> {
    fn cache_get(&mut self, node_id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.nodes[node_id.into()].cache.get(input)
    }

    fn cache_store(&mut self, node_id: NodeId, input: &LayoutInput, layout_output: LayoutOutput) {
        self.nodes[node_id.into()].cache.store(input, layout_output)
    }

    fn cache_clear(&mut self, node_id: NodeId) {
        self.nodes[node_id.into()].cache.clear();
    }
}

// PrintTree impl for TaffyTree
impl<NodeContext> PrintTree for TaffyTree<NodeContext> {
    #[inline(always)]
    fn get_debug_label(&self, node_id: NodeId) -> &'static str {
        let node = &self.nodes[node_id.into()];
        let display = node.style.display;
        let num_children = self.child_count(node_id);

        match (num_children, display) {
            (_, Display::None) => "NONE",
            (0, _) => "LEAF",
            #[cfg(feature = "block_layout")]
            (_, Display::Block) => "BLOCK",
            #[cfg(feature = "block_layout")]
            (_, Display::FlowRoot) => "FLOW-ROOT",
            #[cfg(feature = "flexbox")]
            (_, Display::Flex) => {
                use crate::FlexDirection;
                match node.style.flex_direction {
                    FlexDirection::Row | FlexDirection::RowReverse => "FLEX ROW",
                    FlexDirection::Column | FlexDirection::ColumnReverse => "FLEX COL",
                }
            }
            #[cfg(feature = "grid")]
            (_, Display::Grid) => "GRID",
        }
    }

    #[inline(always)]
    fn get_final_layout(&self, node_id: NodeId) -> Layout {
        if self.config.use_rounding {
            self.nodes[node_id.into()]
                .final_layout
                .as_deref()
                .copied()
                .unwrap_or(NO_FINAL_LAYOUT)
        } else {
            self.nodes[node_id.into()].unrounded_layout
        }
    }
}

/// View over the Taffy tree that holds the tree itself along with a reference to the context
/// and implements LayoutTree. This allows the context to be stored outside of the TaffyTree struct
/// which makes the lifetimes of the context much more flexible.
pub(crate) struct TaffyView<'t, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    /// A reference to the TaffyTree
    pub(crate) taffy: &'t mut TaffyTree<NodeContext>,
    /// The context provided for passing to measure functions if layout is run over this struct
    pub(crate) measure_function: MeasureFunction,
}

impl<NodeContext, MeasureFunction> TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    /// Resolve keyword-based calc-size axes before the final native dispatch.
    async fn compute_child_layout_on_stack(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        #[cfg(feature = "block_layout")] block_ctx: Option<&mut BlockContext<'_>>,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return self.compute_hidden_subtree(node_id);
        }
        let resolved = self.resolve_calc_size_async(node_id, inputs, stack).await;
        self.compute_child_layout_inner_async(
            node_id,
            resolved,
            #[cfg(feature = "block_layout")]
            block_ctx,
            stack,
        )
        .await
    }

    #[inline(always)]
    /// Unified implementation that both `LayoutPartialTree::compute_child_layout`
    /// and `LayoutBlockContainer::compute_block_child_layout` delegate to.
    async fn compute_child_layout_inner_async(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        #[cfg(feature = "block_layout")] block_ctx: Option<&mut BlockContext<'_>>,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput {
        // If RunMode is PerformHiddenLayout then this indicates that an ancestor node is `Display::None`
        // and thus that we should lay out this node using hidden layout regardless of it's own display style.
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            debug_log!("HIDDEN");
            return self.compute_hidden_subtree(node_id);
        }

        #[cfg(feature = "flexbox")]
        if self.taffy.nodes[node_id.into()]
            .style
            .percent_basis_from_parent
            && self.taffy.row_uses_pct_height(node_id)
        {
            let node = &mut self.taffy.nodes[node_id.into()];
            if node.pct_parent_h != Some(inputs.parent_size.height) {
                node.cache.clear();
                node.pct_parent_h = Some(inputs.parent_size.height);
            }
        }

        // We run the following wrapped in "compute_cached_layout", which will check the cache for an entry matching the node and inputs and:
        //   - Return that entry if exists
        //   - Else call the passed closure (below) to compute the result
        //
        // If there was no cache match and a new result needs to be computed then that result will be added to the cache
        compute_cached_layout_async(
            self,
            node_id,
            inputs,
            async |tree, node_id, inputs, stack| {
                let display_mode = tree.taffy.nodes[node_id.into()].style.display;
                let has_children = tree.child_count(node_id) > 0;

                debug_log!(display_mode);
                debug_log_node!(inputs);

                // Dispatch to a layout algorithm based on the node's display style and whether the node has children or not.
                let output = match (display_mode, has_children) {
                    #[cfg(feature = "block_layout")]
                    (Display::Block, true) => {
                        stack
                            .run(|stack| {
                                compute_block_layout_async(tree, node_id, inputs, block_ctx, stack)
                            })
                            .await
                    }
                    #[cfg(feature = "block_layout")]
                    (Display::FlowRoot, true) => {
                        stack
                            .run(|stack| {
                                compute_block_layout_async(tree, node_id, inputs, None, stack)
                            })
                            .await
                    }
                    _ => {
                        tree.compute_other_layout_async(
                            node_id,
                            inputs,
                            display_mode,
                            has_children,
                            stack,
                        )
                        .await
                    }
                };
                tree.finish_child_layout_async(node_id, inputs.run_mode, output, stack)
                    .await
            },
            stack,
        )
        .await
    }

    // KaminIDE patch: this closure's frame waits for the recursive layout of
    // every block descendant, so only the block dispatch stays inline; the
    // inlined entries of the other algorithms must not widen it (a 2052-level
    // box tree on the 8 MiB stack).
    /// Lays out a node that is not a block container with children.
    #[inline(never)]
    async fn compute_other_layout_async(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        display_mode: Display,
        has_children: bool,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput {
        match (display_mode, has_children) {
            (Display::None, _) => self.compute_hidden_subtree(node_id),
            #[cfg(feature = "flexbox")]
            (Display::Flex, true) => {
                stack
                    .run(|stack| compute_flexbox_layout_async(self, node_id, inputs, stack))
                    .await
            }
            #[cfg(feature = "grid")]
            (Display::Grid, true) => {
                stack
                    .run(|stack| compute_grid_layout_async(self, node_id, inputs, stack))
                    .await
            }
            // An empty grid without a measurement context still has explicit
            // tracks and auto-fill sizing. A measured grid leaf stays a leaf.
            #[cfg(feature = "grid")]
            (Display::Grid, false) if !self.taffy.nodes[node_id.into()].has_context => {
                stack
                    .run(|stack| compute_grid_layout_async(self, node_id, inputs, stack))
                    .await
            }
            (_, false) => self.measure_leaf(node_id, inputs, stack).await,
            #[allow(unreachable_patterns)]
            _ => unreachable!("block containers with children are dispatched inline"),
        }
    }

    /// KaminIDE patch: собственное дерево открывает всех потомков, поэтому
    /// скрытый обход не требует рекурсивного вызова partial-tree API.
    fn compute_hidden_subtree(&mut self, root: NodeId) -> LayoutOutput {
        let mut pending = Vec::new();
        let mut current = Some(root);
        while let Some(node) = current.take() {
            let data = &mut self.taffy.nodes[node.into()];
            data.cache.clear();
            data.unrounded_layout = Layout::with_order(0);
            let mut children = self.taffy.children[node.into()].iter();
            loop {
                if let Some(&child) = children.next() {
                    current = Some(child);
                    if children.len() > 0 {
                        pending.push(children);
                    }
                    break;
                }
                match pending.pop() {
                    Some(siblings) => children = siblings,
                    None => break,
                }
            }
        }
        LayoutOutput::HIDDEN
    }

    // KaminIDE patch: the inlined measure function must not widen the frame of
    // `compute_other_layout`, which waits for the recursive layout of flex and
    // grid descendants (a 2052-level box tree on the 8 MiB stack).
    /// Measures a leaf node with its measure function.
    #[inline(never)]
    async fn measure_leaf(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput {
        let node_key = node_id.into();
        let style = &self.taffy.nodes[node_key].style;
        let has_context = self.taffy.nodes[node_key].has_context;
        let node_context = has_context
            .then(|| self.taffy.node_context_data.get_mut(node_key))
            .flatten();
        self.measure_function
            .measure(inputs, node_id, node_context, style, stack)
            .await
    }

    // KaminIDE patch: output adaptation and OOF temporaries must not live
    // in the native frame which waits for recursive container layout.
    /// Adapts a child layout output and records it for the node.
    #[inline(never)]
    async fn finish_child_layout_async(
        &mut self,
        node_id: NodeId,
        run_mode: RunMode,
        output: LayoutOutput,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput {
        let mut output =
            super::kamin_output::adapt(output, &self.taffy.nodes[node_id.into()].style);
        // Still inside the cache-miss closure, so cache entries include OOF placement.
        // Measure passes must not commit hoisted box layouts.
        if run_mode == RunMode::PerformLayout {
            compute_oof_layout_async(self, node_id, &mut output, stack).await;
        }
        output
    }
}

// TraversePartialTree impl for TaffyView
impl<NodeContext, MeasureFunction> TraversePartialTree
    for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type ChildIter<'a>
        = TaffyTreeChildIter<'a>
    where
        Self: 'a;

    #[inline(always)]
    fn child_ids(&self, parent_node_id: NodeId) -> Self::ChildIter<'_> {
        self.taffy.child_ids(parent_node_id)
    }

    #[inline(always)]
    fn child_count(&self, parent_node_id: NodeId) -> usize {
        self.taffy.child_count(parent_node_id)
    }

    #[inline(always)]
    fn get_child_id(&self, parent_node_id: NodeId, child_index: usize) -> NodeId {
        self.taffy.get_child_id(parent_node_id, child_index)
    }
}

// TraverseTree impl for TaffyView
impl<NodeContext, MeasureFunction> TraverseTree for TaffyView<'_, NodeContext, MeasureFunction> where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>
{
}

// LayoutPartialTree impl for TaffyView
impl<NodeContext, MeasureFunction> LayoutPartialTree for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type CoreContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;

    type CustomIdent = DefaultCheapStr;

    #[inline(always)]
    fn get_core_container_style(&self, node_id: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.taffy.nodes[node_id.into()].style
    }

    #[inline(always)]
    fn set_unrounded_layout(&mut self, node_id: NodeId, layout: &Layout) {
        self.taffy.nodes[node_id.into()].unrounded_layout = *layout;
    }

    #[inline(always)]
    fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
        calc_value(val, basis)
    }

    #[inline(always)]
    fn compute_child_layout(&mut self, node_id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        reblessive::Stack::new()
            .enter(|stack| self.compute_child_layout_async(node_id, inputs, stack))
            .finish()
    }
    /// KaminIDE patch: Suspend a descendant without consuming a native frame.
    fn compute_child_layout_async<'a>(
        &'a mut self,
        node: NodeId,
        inputs: LayoutInput,
        stack: &'a mut reblessive::Stk,
    ) -> impl core::future::Future<Output = LayoutOutput> + 'a {
        async move {
            stack
                .run(|stack| {
                    self.compute_child_layout_on_stack(
                        node,
                        inputs,
                        #[cfg(feature = "block_layout")]
                        None,
                        stack,
                    )
                })
                .await
        }
    }
}

impl<NodeContext, MeasureFunction> LayoutContainingBlock
    for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type OofItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    #[inline(always)]
    fn get_oof_item_style(&self, node_id: NodeId) -> Self::OofItemStyle<'_> {
        &self.taffy.nodes[node_id.into()].style
    }

    #[inline(always)]
    fn clear_hoisted_children(&mut self, node_id: NodeId) {
        self.taffy.nodes[node_id.into()].hoisted_children.clear();
    }

    #[inline(always)]
    fn add_hoisted_children(&mut self, node_id: NodeId, hoisted: &[NodeId]) {
        self.taffy.nodes[node_id.into()]
            .hoisted_children
            .extend_from_slice(hoisted);
    }

    #[inline(always)]
    fn get_detailed_layout_info(&self, node_id: NodeId) -> &DetailedLayoutInfo {
        &self.taffy.nodes[node_id.into()].detailed_layout_info
    }
}

impl<NodeContext, MeasureFunction> CacheTree for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    fn cache_get(&mut self, node_id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.taffy.nodes[node_id.into()].cache.get(input)
    }

    fn cache_store(&mut self, node_id: NodeId, input: &LayoutInput, layout_output: LayoutOutput) {
        self.taffy.nodes[node_id.into()]
            .cache
            .store(input, layout_output)
    }

    fn cache_clear(&mut self, node_id: NodeId) {
        self.taffy.nodes[node_id.into()].cache.clear();
    }
}

#[cfg(feature = "block_layout")]
impl<NodeContext, MeasureFunction> LayoutBlockContainer
    for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type BlockContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type BlockItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    #[inline(always)]
    fn get_block_container_style(&self, node_id: NodeId) -> Self::BlockContainerStyle<'_> {
        self.get_core_container_style(node_id)
    }

    #[inline(always)]
    fn get_block_child_style(&self, child_node_id: NodeId) -> Self::BlockItemStyle<'_> {
        self.get_core_container_style(child_node_id)
    }

    #[inline(always)]
    fn compute_block_child_layout(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> LayoutOutput {
        reblessive::Stack::new()
            .enter(|stack| self.compute_block_child_layout_async(node_id, inputs, block_ctx, stack))
            .finish()
    }
    /// KaminIDE patch: Suspend a descendant without consuming a native frame.
    fn compute_block_child_layout_async<'a, 'ctx: 'a>(
        &'a mut self,
        node: NodeId,
        inputs: LayoutInput,
        context: Option<&'a mut BlockContext<'ctx>>,
        stack: &'a mut reblessive::Stk,
    ) -> impl core::future::Future<Output = LayoutOutput> + 'a {
        async move {
            stack
                .run(|stack| self.compute_child_layout_on_stack(node, inputs, context, stack))
                .await
        }
    }
}

#[cfg(feature = "flexbox")]
impl<NodeContext, MeasureFunction> LayoutFlexboxContainer
    for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type FlexboxContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    #[inline(always)]
    fn get_flexbox_container_style(&self, node_id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        &self.taffy.nodes[node_id.into()].style
    }

    #[inline(always)]
    fn get_flexbox_child_style(&self, child_node_id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.taffy.nodes[child_node_id.into()].style
    }

    // KaminIDE patch: store geometry only after the final placement pass.
    fn set_detailed_flex_info(&mut self, node_id: NodeId, info: crate::tree::DetailedFlexInfo) {
        self.taffy.nodes[node_id.into()].detailed_layout_info =
            DetailedLayoutInfo::Flex(Box::new(info));
    }

    fn flex_fragmentation_axis(&self, node: NodeId) -> (crate::geometry::AbsoluteAxis, bool) {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .flex_fragmentation_axis
    }

    fn flex_item_fragmentation_consumed(&self, node: NodeId) -> f32 {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .flex_item_consumed
    }

    fn flex_item_fragmentation_offset(&self, node: NodeId) -> f32 {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .flex_item_offset
    }

    fn flex_line_fragmentation_growth(&self, node: NodeId, line: usize) -> f32 {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .flex_line_growth
            .get(line)
            .copied()
            .unwrap_or(0.0)
    }

    fn flex_item_fragmentation_line(&self, node: NodeId) -> Option<usize> {
        self.taffy.nodes[node.into()].fragmentation().flex_item_line
    }

    fn fragmentation_monolithic(&self, node: NodeId) -> bool {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .fragmentation_monolithic
    }

    fn flex_fragmentation(&self, node: NodeId) -> bool {
        self.taffy.nodes[node.into()]
            .fragmentation()
            .flex_fragmentation
    }
}

#[cfg(feature = "grid")]
impl<NodeContext, MeasureFunction> LayoutGridContainer
    for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    type GridContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    #[inline(always)]
    fn get_grid_container_style(&self, node_id: NodeId) -> Self::GridContainerStyle<'_> {
        &self.taffy.nodes[node_id.into()].style
    }

    #[inline(always)]
    fn get_grid_child_style(&self, child_node_id: NodeId) -> Self::GridItemStyle<'_> {
        &self.taffy.nodes[child_node_id.into()].style
    }

    fn get_subgrid_tracks(&self, node_id: NodeId) -> Option<crate::style::SubgridTracks> {
        self.taffy.nodes[node_id.into()]
            .subgrid_tracks
            .as_deref()
            .cloned()
    }

    fn set_content_shift(&mut self, node_id: NodeId, dy: f32) {
        self.taffy.nodes[node_id.into()].content_shift = dy;
    }

    fn grid_track_fragmentation_growth(
        &self,
        node: NodeId,
        axis: crate::geometry::AbsoluteAxis,
        track: usize,
    ) -> f32 {
        let (growth_axis, growth) = &self.taffy.nodes[node.into()]
            .fragmentation()
            .grid_track_growth;
        if *growth_axis == axis {
            growth.get(track).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    fn set_subgrid_tracks(&mut self, node_id: NodeId, tracks: Option<crate::style::SubgridTracks>) {
        let node = &mut self.taffy.nodes[node_id.into()];
        if node.subgrid_tracks.as_deref() != tracks.as_ref() {
            node.subgrid_tracks = tracks.map(Box::new);
            node.cache.clear();
        }
    }

    #[inline(always)]
    fn set_detailed_grid_info(&mut self, node_id: NodeId, detailed_grid_info: DetailedGridInfo) {
        self.taffy.nodes[node_id.into()].detailed_layout_info =
            DetailedLayoutInfo::Grid(Box::new(detailed_grid_info));
    }
}

// RoundTree impl for TaffyView
impl<NodeContext, MeasureFunction> RoundTree for TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    #[inline(always)]
    fn get_unrounded_layout(&self, node: NodeId) -> Layout {
        self.taffy.nodes[node.into()].unrounded_layout
    }

    #[inline(always)]
    fn set_final_layout(&mut self, node_id: NodeId, layout: &Layout) {
        **self.taffy.nodes[node_id.into()]
            .final_layout
            .get_or_insert_with(|| Box::new(NO_FINAL_LAYOUT)) = *layout;
    }

    #[inline(always)]
    fn is_out_of_flow(&self, node_id: NodeId) -> bool {
        let node = &self.taffy.nodes[node_id.into()];
        node.style.position.is_out_of_flow() && node.style.display != crate::style::Display::None
    }

    #[inline(always)]
    fn hoisted_child_count(&self, node_id: NodeId) -> usize {
        self.taffy.nodes[node_id.into()].hoisted_children.len()
    }

    #[inline(always)]
    fn get_hoisted_child_id(&self, node_id: NodeId, index: usize) -> NodeId {
        self.taffy.nodes[node_id.into()].hoisted_children[index]
    }
}

#[allow(clippy::iter_cloned_collect)] // due to no-std support, we need to use `iter_cloned` instead of `collect`
impl<NodeContext> TaffyTree<NodeContext> {
    #[cfg(feature = "flexbox")]
    /// KaminIDE patch: Whether a row-flex descendant requires a percentage height basis.
    fn row_uses_pct_height(&self, node: NodeId) -> bool {
        use crate::style::{ExpandedDimension, ExpandedLengthPercentageAuto};
        let preferred = |value: crate::style::Dimension| match value.expand() {
            ExpandedDimension::Percent(_) | ExpandedDimension::FitContentPercent(_) => true,
            #[cfg(feature = "calc")]
            ExpandedDimension::Calc(_) => true,
            _ => false,
        };
        let limit = |value: crate::style::LengthPercentageAuto| match value.expand() {
            ExpandedLengthPercentageAuto::Percent(_) => true,
            #[cfg(feature = "calc")]
            ExpandedLengthPercentageAuto::Calc(_) => true,
            _ => false,
        };
        let mut pending = Vec::new();
        let mut children = self.children[node.into()].iter();
        loop {
            if let Some(&child) = children.next() {
                let style = &self.nodes[child.into()].style;
                if preferred(style.size.height)
                    || limit(style.min_size.height)
                    || limit(style.max_size.height)
                {
                    return true;
                }
                if style.percent_basis_from_parent {
                    if children.len() > 0 {
                        pending.push(children);
                    }
                    children = self.children[child.into()].iter();
                }
            } else {
                match pending.pop() {
                    Some(siblings) => children = siblings,
                    None => return false,
                }
            }
        }
    }

    /// Creates a new [`TaffyTree`]
    ///
    /// The default capacity of a [`TaffyTree`] is 16 nodes.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    /// Creates a new [`TaffyTree`] that can store `capacity` nodes before reallocation
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        TaffyTree {
            // TODO: make this method const upstream,
            // so constructors here can be const
            nodes: SlotMap::with_capacity(capacity),
            children: SlotMap::with_capacity(capacity),
            parents: SlotMap::with_capacity(capacity),
            node_context_data: SecondaryMap::with_capacity(capacity),
            config: TaffyConfig::default(),
            #[cfg(feature = "std")]
            calc: Default::default(),
        }
    }

    /// KaminIDE patch: make this tree's calc registry current until the guard drops:
    /// `calc_handle` registers into it while the host converts styles.
    #[cfg(feature = "std")]
    pub fn calc_scope(&self) -> super::kamin_calc::CalcScope {
        super::kamin_calc::enter(&self.calc)
    }

    /// KaminIDE patch: resolve calc handles of `source`'s styles — for a tree built
    /// from copies of `source`'s styles (a measurement snapshot). The registry
    /// is shared, so it lives while either tree does.
    #[cfg(feature = "std")]
    pub fn share_calc_registry<C>(&mut self, source: &TaffyTree<C>) {
        self.calc = source.calc.clone();
    }

    /// Enable rounding of layout values. Rounding is enabled by default.
    pub fn enable_rounding(&mut self) {
        self.config.use_rounding = true;
    }

    /// Disable rounding of layout values. Rounding is enabled by default.
    pub fn disable_rounding(&mut self) {
        self.config.use_rounding = false;
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the node of the new node
    pub fn new_leaf(&mut self, layout: impl Into<SharedStyle>) -> TaffyResult<NodeId> {
        let id = self.nodes.insert(NodeData::new(layout.into()));
        let _ = self.children.insert(new_vec_with_capacity(0));
        let _ = self.parents.insert(None);

        Ok(id.into())
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the [`NodeId`] of the new node
    ///
    /// Creates and adds a new leaf node with a supplied context
    pub fn new_leaf_with_context(
        &mut self,
        layout: impl Into<SharedStyle>,
        context: NodeContext,
    ) -> TaffyResult<NodeId> {
        let mut data = NodeData::new(layout.into());
        data.has_context = true;

        let id = self.nodes.insert(data);
        self.node_context_data.insert(id, context);

        let _ = self.children.insert(new_vec_with_capacity(0));
        let _ = self.parents.insert(None);

        Ok(id.into())
    }

    /// Creates and adds a new node, which may have any number of `children`
    pub fn new_with_children(
        &mut self,
        layout: impl Into<SharedStyle>,
        children: &[NodeId],
    ) -> TaffyResult<NodeId> {
        let id = NodeId::from(self.nodes.insert(NodeData::new(layout.into())));

        for child in children {
            self.parents[(*child).into()] = Some(id);
        }

        let _ = self
            .children
            .insert(children.iter().copied().collect::<_>());
        let _ = self.parents.insert(None);

        Ok(id)
    }

    /// Drops all nodes in the tree
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.children.clear();
        self.parents.clear();
        // A context belongs to the node that owns it. Leaving it behind keeps arbitrary
        // user data -- for a measure function, a boxed closure and everything it
        // captures -- alive after the node it belonged to is gone.
        self.node_context_data.clear();
        #[cfg(feature = "std")]
        self.calc
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }

    /// KaminIDE patch: the children of `parent`, without allocating.
    pub fn children_slice(&self, parent: NodeId) -> &[NodeId] {
        self.children
            .get(parent.into())
            .map_or(&[], |children| children.as_slice())
    }

    /// KaminIDE patch: whether `node` carries a context (a measured leaf).
    pub fn has_node_context(&self, node: NodeId) -> bool {
        self.nodes
            .get(node.into())
            .is_some_and(|data| data.has_context)
    }

    /// KaminIDE patch: turn `node` into a fresh node with `style`, `children` and
    /// `context`, reusing its storage. The node leaves its previous parent
    /// (it becomes a root, the parent's list is left as is), its previous
    /// children that still point at it become roots, and its cache is empty.
    /// Like `new_with_children`, the new children are adopted without editing
    /// their previous parents' lists.
    pub fn reset_node(
        &mut self,
        node: NodeId,
        style: impl Into<SharedStyle>,
        children: &[NodeId],
        context: Option<NodeContext>,
    ) -> TaffyResult<()> {
        let style = style.into();
        let key = node.into();
        for child in self.children[key].iter() {
            if self.parents[(*child).into()] == Some(node) {
                self.parents[(*child).into()] = None;
            }
        }
        self.parents[key] = None;
        let data = &mut self.nodes[key];
        *data = NodeData::new(style);
        data.has_context = context.is_some();
        match context {
            Some(context) => {
                self.node_context_data.insert(key, context);
            }
            None => {
                self.node_context_data.remove(key);
            }
        }
        for child in children {
            self.parents[(*child).into()] = Some(node);
        }
        let list = &mut self.children[key];
        list.clear();
        list.extend_from_slice(children);
        Ok(())
    }

    /// KaminIDE patch: drop every node for which `keep` is false, in one pass.
    ///
    /// Kept nodes keep their style, children, context and layout cache. A kept
    /// node whose parent is dropped becomes a root; a kept node that loses a
    /// child is marked dirty. Unlike `remove`, a dropped node never unlinks a
    /// child that another node has adopted since (`new_with_children` moves a
    /// child without editing its previous parent's list). The calc registry is
    /// kept: its pairs are interned by value, so handles stay valid for the
    /// kept styles.
    pub fn retain_nodes(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        let dropped: Vec<DefaultKey> = self
            .nodes
            .keys()
            .filter(|key| !keep(NodeId::from(*key)))
            .collect();
        if dropped.is_empty() {
            return;
        }
        for key in &dropped {
            let _ = self.children.remove(*key);
            let _ = self.parents.remove(*key);
            let _ = self.nodes.remove(*key);
            let _ = self.node_context_data.remove(*key);
        }
        let mut orphaned = Vec::new();
        for (key, parent) in self.parents.iter_mut() {
            if parent.is_some_and(|p| !self.nodes.contains_key(p.into())) {
                *parent = None;
            }
            let children = &mut self.children[key];
            let before = children.len();
            children.retain(|child| self.nodes.contains_key((*child).into()));
            if children.len() != before {
                orphaned.push(NodeId::from(key));
            }
        }
        for node in orphaned {
            let _ = self.mark_dirty(node);
        }
    }

    /// Remove a specific node from the tree and drop it
    ///
    /// Returns the id of the node removed.
    pub fn remove(&mut self, node: NodeId) -> TaffyResult<NodeId> {
        let key = node.into();
        if let Some(parent) = self.parents[key] {
            if let Some(children) = self.children.get_mut(parent.into()) {
                children.retain(|f| *f != node);
            }
            self.mark_dirty(parent)?;
        }

        // Remove "parent" references to a node when removing that node
        if let Some(children) = self.children.get(key) {
            for child in children.iter().copied() {
                self.parents[child.into()] = None;
            }
        }

        let _ = self.children.remove(key);
        let _ = self.parents.remove(key);
        let _ = self.nodes.remove(key);
        let _ = self.node_context_data.remove(key);

        Ok(node)
    }

    /// Sets the context data associated with the node
    #[inline]
    pub fn set_node_context(
        &mut self,
        node: NodeId,
        measure: Option<NodeContext>,
    ) -> TaffyResult<()> {
        let key = node.into();
        if let Some(measure) = measure {
            self.nodes[key].has_context = true;
            self.node_context_data.insert(key, measure);
        } else {
            self.nodes[key].has_context = false;
            self.node_context_data.remove(key);
        }

        self.mark_dirty(node)?;

        Ok(())
    }

    /// Gets a reference to the the context data associated with the node
    #[inline]
    pub fn get_node_context(&self, node: NodeId) -> Option<&NodeContext> {
        self.node_context_data.get(node.into())
    }

    /// Gets a mutable reference to the the context data associated with the node
    #[inline]
    pub fn get_node_context_mut(&mut self, node: NodeId) -> Option<&mut NodeContext> {
        self.node_context_data.get_mut(node.into())
    }

    /// Gets mutable references to the the context data associated with the nodes. All keys must be valid and disjoint, otherwise None is returned.
    pub fn get_disjoint_node_context_mut<const N: usize>(
        &mut self,
        keys: [NodeId; N],
    ) -> Option<[&mut NodeContext; N]> {
        self.node_context_data
            .get_disjoint_mut(keys.map(|k| k.into()))
    }

    /// Adds a `child` node under the supplied `parent`
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) -> TaffyResult<()> {
        let parent_key = parent.into();
        let child_key = child.into();
        self.parents[child_key] = Some(parent);
        self.children[parent_key].push(child);
        self.mark_dirty(parent)?;

        Ok(())
    }

    /// Inserts a `child` node at the given `child_index` under the supplied `parent`, shifting all children after it to the right.
    pub fn insert_child_at_index(
        &mut self,
        parent: NodeId,
        child_index: usize,
        child: NodeId,
    ) -> TaffyResult<()> {
        let parent_key = parent.into();

        let child_count = self.children[parent_key].len();
        if child_index > child_count {
            return Err(TaffyError::ChildIndexOutOfBounds {
                parent,
                child_index,
                child_count,
            });
        }

        self.parents[child.into()] = Some(parent);
        self.children[parent_key].insert(child_index, child);
        self.mark_dirty(parent)?;

        Ok(())
    }

    /// Directly sets the `children` of the supplied `parent`
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) -> TaffyResult<()> {
        let parent_key = parent.into();

        // Remove node as parent from all its current children.
        for child in &self.children[parent_key] {
            self.parents[(*child).into()] = None;
        }

        // Build up relation node <-> child
        for &child in children {
            // Remove child from previous parent
            if let Some(previous_parent) = self.parents[child.into()] {
                self.remove_child(previous_parent, child).unwrap();
            }
            self.parents[child.into()] = Some(parent);
        }

        let parent_children = &mut self.children[parent_key];
        parent_children.clear();
        children
            .iter()
            .for_each(|child| parent_children.push(*child));

        self.mark_dirty(parent)?;

        Ok(())
    }

    /// Removes the `child` of the parent `node`
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> TaffyResult<NodeId> {
        let index = self.children[parent.into()]
            .iter()
            .position(|n| *n == child)
            .unwrap();
        self.remove_child_at_index(parent, index)
    }

    /// Removes the child at the given `index` from the `parent`
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn remove_child_at_index(
        &mut self,
        parent: NodeId,
        child_index: usize,
    ) -> TaffyResult<NodeId> {
        let parent_key = parent.into();
        let child_count = self.children[parent_key].len();
        if child_index >= child_count {
            return Err(TaffyError::ChildIndexOutOfBounds {
                parent,
                child_index,
                child_count,
            });
        }

        let child = self.children[parent_key].remove(child_index);
        self.parents[child.into()] = None;

        self.mark_dirty(parent)?;

        Ok(child)
    }

    /// Removes children at the given range from the `parent`
    ///
    /// Children are not removed from the tree entirely, they are simply no longer attached to their previous parent.
    ///
    /// Function will panic if given range is invalid. See [`core::slice::range`]
    pub fn remove_children_range<R>(&mut self, parent: NodeId, range: R) -> TaffyResult<()>
    where
        R: core::ops::RangeBounds<usize>,
    {
        let parent_key = parent.into();
        for child in self.children[parent_key].drain(range) {
            self.parents[child.into()] = None;
        }

        self.mark_dirty(parent)?;
        Ok(())
    }

    /// Replaces the child at the given `child_index` from the `parent` node with the new `child` node
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn replace_child_at_index(
        &mut self,
        parent: NodeId,
        child_index: usize,
        new_child: NodeId,
    ) -> TaffyResult<NodeId> {
        let parent_key = parent.into();

        let child_count = self.children[parent_key].len();
        if child_index >= child_count {
            return Err(TaffyError::ChildIndexOutOfBounds {
                parent,
                child_index,
                child_count,
            });
        }

        self.parents[new_child.into()] = Some(parent);
        let old_child = core::mem::replace(&mut self.children[parent_key][child_index], new_child);
        self.parents[old_child.into()] = None;

        self.mark_dirty(parent)?;

        Ok(old_child)
    }

    /// Returns the child node of the parent `node` at the provided `child_index`
    #[inline]
    pub fn child_at_index(&self, parent: NodeId, child_index: usize) -> TaffyResult<NodeId> {
        let parent_key = parent.into();
        let child_count = self.children[parent_key].len();
        if child_index >= child_count {
            return Err(TaffyError::ChildIndexOutOfBounds {
                parent,
                child_index,
                child_count,
            });
        }

        Ok(self.children[parent_key][child_index])
    }

    /// Returns the total number of nodes in the tree
    #[inline]
    pub fn total_node_count(&self) -> usize {
        self.nodes.len()
    }

    /// KaminIDE patch: whether the next node insertion has to grow the node storage.
    #[inline]
    pub fn node_storage_full(&self) -> bool {
        self.nodes.len() >= self.nodes.capacity()
    }

    /// KaminIDE patch: grow the node storage to hold `expected` nodes at once. A node
    /// is about 2 KiB, so doubling up to a large tree copies the whole tree again.
    pub fn reserve_nodes(&mut self, expected: usize) {
        let additional = expected.saturating_sub(self.nodes.len());
        if expected > self.nodes.capacity() {
            self.nodes.reserve(additional);
            self.children.reserve(additional);
            self.parents.reserve(additional);
        }
    }

    /// Returns the `NodeId` of the parent node of the specified node (if it exists)
    ///
    /// - Return None if the specified node has no parent
    /// - Panics if the specified node does not exist
    #[inline]
    pub fn parent(&self, child_id: NodeId) -> Option<NodeId> {
        self.parents[child_id.into()]
    }

    /// Returns a list of children that belong to the parent node
    pub fn children(&self, parent: NodeId) -> TaffyResult<Vec<NodeId>> {
        Ok(self.children[parent.into()].clone())
    }

    /// Returns the out-of-flow (absolute/fixed) boxes whose containing block is `node`, as
    /// recorded by the last layout. These boxes are laid out by `node` rather than by their
    /// parent, and their [`Layout::location`] is relative to `node`.
    pub fn hoisted_children(&self, node: NodeId) -> TaffyResult<&[NodeId]> {
        Ok(&self.nodes[node.into()].hoisted_children)
    }

    /// Sets the [`Style`] of the provided `node`
    #[inline]
    pub fn set_style(&mut self, node: NodeId, style: impl Into<SharedStyle>) -> TaffyResult<()> {
        self.nodes[node.into()].style = style.into();
        self.mark_dirty(node)?;
        Ok(())
    }

    /// Gets the [`Style`] of the provided `node`
    #[inline]
    pub fn style(&self, node: NodeId) -> TaffyResult<&Style> {
        Ok(&self.nodes[node.into()].style)
    }

    /// KaminIDE patch: whether the style of `node` equals `style`, by allocation or value.
    #[inline]
    pub fn style_equals(&self, node: NodeId, style: &SharedStyle) -> bool {
        self.nodes
            .get(node.into())
            .is_some_and(|data| SharedStyle::ptr_eq(&data.style, style) || *data.style == **style)
    }

    /// KaminIDE patch: give `node` the shared allocation of `style`, which must equal the
    /// node's style. Unlike `set_style`, the node keeps its layout cache: its input is the
    /// same value. Returns false and changes nothing when the styles differ.
    pub fn adopt_equal_style(&mut self, node: NodeId, style: &SharedStyle) -> bool {
        let Some(data) = self.nodes.get_mut(node.into()) else {
            return false;
        };
        if SharedStyle::ptr_eq(&data.style, style) {
            return true;
        }
        if *data.style != **style {
            return false;
        }
        data.style = style.clone();
        true
    }

    /// KaminIDE patch: the shared style of `node`, to give another node the same style.
    #[inline]
    pub fn shared_style(&self, node: NodeId) -> TaffyResult<SharedStyle> {
        Ok(self.nodes[node.into()].style.clone())
    }

    /// Return this node layout relative to its parent
    #[inline]
    pub fn layout(&self, node: NodeId) -> TaffyResult<&Layout> {
        if self.config.use_rounding {
            Ok(self.nodes[node.into()]
                .final_layout
                .as_deref()
                .unwrap_or(&NO_FINAL_LAYOUT))
        } else {
            Ok(&self.nodes[node.into()].unrounded_layout)
        }
    }

    /// KaminIDE patch: y offset of the node's children inside its box (a
    /// `vertical-align: baseline` table cell); add it to the node's origin when
    /// placing its children.
    #[inline]
    pub fn content_shift(&self, node: NodeId) -> f32 {
        self.nodes[node.into()].content_shift
    }

    /// Returns this node layout with unrounded values relative to its parent.
    #[inline]
    pub fn unrounded_layout(&self, node: NodeId) -> &Layout {
        &self.nodes[node.into()].unrounded_layout
    }

    /// Get the "detailed layout info" for a node.
    ///
    /// Currently this is only implemented for CSS Grid containers where it contains
    /// the computed size of each grid track and the computed placement of each grid item
    #[inline]
    pub fn detailed_layout_info(&self, node_id: NodeId) -> &DetailedLayoutInfo {
        &self.nodes[node_id.into()].detailed_layout_info
    }

    /// Marks the layout of this node and its ancestors as outdated
    pub fn mark_dirty(&mut self, node: NodeId) -> TaffyResult<()> {
        let mut node_key = node.into();
        loop {
            if matches!(self.nodes[node_key].mark_dirty(), ClearState::AlreadyEmpty) {
                // Уже грязный узел ранее инвалидировал своих предков.
                break;
            }
            match self.parents.get(node_key) {
                Some(Some(parent)) => node_key = (*parent).into(),
                _ => break,
            }
        }

        Ok(())
    }

    /// Indicates whether the layout of this node needs to be recomputed
    #[inline]
    pub fn dirty(&self, node: NodeId) -> TaffyResult<bool> {
        Ok(self.nodes[node.into()].cache.is_empty())
    }

    /// Updates the stored layout of the provided `node` and its children
    pub fn compute_layout_with_measure<MeasureFunction>(
        &mut self,
        node_id: NodeId,
        available_space: Size<AvailableSpace>,
        measure_function: MeasureFunction,
    ) -> Result<(), TaffyError>
    where
        MeasureFunction:
            FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
    {
        reblessive::Stack::new()
            .enter(|stack| {
                self.compute_layout_with_measure_async(
                    node_id,
                    available_space,
                    crate::tree::SyncMeasure(measure_function),
                    stack,
                )
            })
            .finish()
    }

    /// KaminIDE patch: query one intrinsic axis without performing or replacing final layout.
    pub fn compute_size_with_measure<MeasureFunction>(
        &mut self,
        node_id: NodeId,
        available_space: Size<AvailableSpace>,
        axis: crate::RequestedAxis,
        measure_function: MeasureFunction,
    ) -> LayoutOutput
    where
        MeasureFunction:
            FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
    {
        reblessive::Stack::new()
            .enter(|stack| {
                self.compute_size_with_measure_async(
                    node_id,
                    available_space,
                    axis,
                    crate::tree::SyncMeasure(measure_function),
                    stack,
                )
            })
            .finish()
    }

    /// Updates the stored layout of the provided `node` and its children
    pub fn compute_layout(
        &mut self,
        node: NodeId,
        available_space: Size<AvailableSpace>,
    ) -> Result<(), TaffyError> {
        self.compute_layout_with_measure(node, available_space, |inputs, _, _, style| {
            compute_leaf_layout(inputs, style, calc_value, |_, _| Size::ZERO)
        })
    }

    /// KaminIDE patch: use a caller-owned continuation stack and lending measurer.
    pub async fn compute_layout_with_measure_async<MeasureFunction>(
        &mut self,
        node_id: NodeId,
        available_space: Size<AvailableSpace>,
        measure_function: MeasureFunction,
        stack: &mut reblessive::Stk,
    ) -> Result<(), TaffyError>
    where
        MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
    {
        let use_rounding = self.config.use_rounding;
        #[cfg(feature = "std")]
        let _calc = self.calc_scope();
        let mut taffy_view = TaffyView {
            taffy: self,
            measure_function,
        };
        compute_root_layout_async(&mut taffy_view, node_id, available_space, stack).await;
        if use_rounding {
            round_layout(&mut taffy_view, node_id);
        }
        Ok(())
    }
    /// KaminIDE patch: use a caller-owned continuation stack and lending measurer.
    pub async fn compute_size_with_measure_async<MeasureFunction>(
        &mut self,
        node_id: NodeId,
        available_space: Size<AvailableSpace>,
        axis: crate::RequestedAxis,
        measure_function: MeasureFunction,
        stack: &mut reblessive::Stk,
    ) -> LayoutOutput
    where
        MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
    {
        #[cfg(feature = "std")]
        let _calc = self.calc_scope();
        let mut view = TaffyView {
            taffy: self,
            measure_function,
        };
        LayoutPartialTree::compute_child_layout_async(
            &mut view,
            node_id,
            LayoutInput {
                run_mode: crate::RunMode::ComputeSize,
                sizing_mode: crate::SizingMode::InherentSize,
                axis,
                known_dimensions: Size::NONE,
                known_dimensions_are_definite: Size {
                    width: false,
                    height: false,
                },
                parent_size: available_space.into_options(),
                available_space,
                vertical_margins_are_collapsible: crate::geometry::Line::FALSE,
            },
            stack,
        )
        .await
    }
    /// Prints a debug representation of the tree's layout
    #[cfg(feature = "std")]
    pub fn print_tree(&mut self, root: NodeId) {
        crate::util::print_tree(self, root)
    }

    /// Returns an instance of LayoutTree representing the TaffyTree
    #[cfg(test)]
    pub(crate) fn as_layout_tree(&mut self) -> impl LayoutPartialTree + CacheTree + '_ {
        TaffyView {
            taffy: self,
            measure_function: crate::tree::SyncMeasure(
                |inputs: LayoutInput, _: NodeId, _: Option<&mut NodeContext>, style: &Style| {
                    compute_leaf_layout(inputs, style, calc_value, |_, _| Size::ZERO)
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::style::{Dimension, Display, FlexDirection};
    use crate::style_helpers::*;
    use crate::util::sys;

    fn size_measure_function(
        inputs: LayoutInput,
        _node_id: NodeId,
        node_context: Option<&mut Size<f32>>,
        style: &Style,
    ) -> LayoutOutput {
        compute_leaf_layout(
            inputs,
            style,
            |_, _| 0.0,
            |known_dimensions, _available_space| {
                known_dimensions.unwrap_or(node_context.cloned().unwrap_or(Size::ZERO))
            },
        )
    }

    #[test]
    #[cfg(all(feature = "flexbox", feature = "grid"))]
    fn fragmentation_inputs_allocate_only_when_a_setter_changes_them() {
        let mut tree = TaffyTree::<()>::new();
        let node = tree.new_leaf(Style::default()).unwrap();
        let block = |tree: &TaffyTree<()>| tree.nodes[node.into()].fragmentation.is_some();
        // Setting the defaults keeps the node without a block.
        tree.set_flex_fragmentation(node, false).unwrap();
        tree.set_fragmentation_monolithic(node, false).unwrap();
        tree.set_flex_item_fragmentation_line(node, None).unwrap();
        tree.set_flex_item_fragmentation_offset(node, 0.0).unwrap();
        tree.set_flex_fragmentation_axis(node, crate::geometry::AbsoluteAxis::Vertical, false)
            .unwrap();
        assert!(!block(&tree));
        assert_eq!(
            *tree.nodes[node.into()].fragmentation(),
            NodeFragmentation::NONE
        );
        // A changed input allocates the block and reads back; the others keep their defaults.
        tree.set_flex_item_fragmentation_consumed(node, 4.0)
            .unwrap();
        assert!(block(&tree));
        let data = tree.nodes[node.into()].fragmentation();
        assert_eq!(data.flex_item_consumed, 4.0);
        assert_eq!(data.flex_item_line, None);
        assert!(data.grid_track_growth.1.is_empty());
        // Every node pays one pointer instead of the inline inputs (64-bit: 432 -> 360 bytes).
        #[cfg(target_pointer_width = "64")]
        assert!(core::mem::size_of::<NodeData>() <= 360);
    }

    #[test]
    fn intrinsic_axis_query_preserves_final_geometry_and_cache_purpose() {
        let mut tree = TaffyTree::<()>::new();
        let root = tree.new_leaf(Style::default()).unwrap();
        let mut measure = |input: LayoutInput, _: NodeId, _: Option<&mut ()>, style: &Style| {
            compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| Size {
                    width: known.width.unwrap_or(match available.width {
                        AvailableSpace::MinContent => 20.0,
                        _ => 60.0,
                    }),
                    height: if input.axis == crate::RequestedAxis::Horizontal {
                        0.0
                    } else {
                        10.0
                    },
                },
            )
        };
        let max = Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        };
        tree.compute_layout_with_measure(root, max, &mut measure)
            .unwrap();
        let final_layout = *tree.layout(root).unwrap();
        let intrinsic = tree.compute_size_with_measure(
            root,
            Size {
                width: AvailableSpace::MinContent,
                ..max
            },
            crate::RequestedAxis::Horizontal,
            &mut measure,
        );
        assert_eq!(intrinsic.size.width, 20.0);
        assert_eq!(*tree.layout(root).unwrap(), final_layout);
        tree.compute_layout_with_measure(root, max, &mut measure)
            .unwrap();
        assert_eq!(
            tree.layout(root).unwrap().size,
            Size {
                width: 60.0,
                height: 10.0
            }
        );
    }

    #[test]
    fn new_should_allocate_default_capacity() {
        const DEFAULT_CAPACITY: usize = 16; // This is the capacity defined in the `impl Default`
        let taffy: TaffyTree<()> = TaffyTree::new();

        assert!(taffy.children.capacity() >= DEFAULT_CAPACITY);
        assert!(taffy.parents.capacity() >= DEFAULT_CAPACITY);
        assert!(taffy.nodes.capacity() >= DEFAULT_CAPACITY);
    }

    #[test]
    fn test_with_capacity() {
        const CAPACITY: usize = 8;
        let taffy: TaffyTree<()> = TaffyTree::with_capacity(CAPACITY);

        assert!(taffy.children.capacity() >= CAPACITY);
        assert!(taffy.parents.capacity() >= CAPACITY);
        assert!(taffy.nodes.capacity() >= CAPACITY);
    }

    #[test]
    fn test_new_leaf() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let res = taffy.new_leaf(Style::default());
        assert!(res.is_ok());
        let node = res.unwrap();

        // node should be in the taffy tree and have no children
        assert!(taffy.child_count(node) == 0);
    }

    #[test]
    fn new_leaf_with_context() {
        let mut taffy: TaffyTree<Size<f32>> = TaffyTree::new();

        let res = taffy.new_leaf_with_context(Style::default(), Size::ZERO);
        assert!(res.is_ok());
        let node = res.unwrap();

        // node should be in the taffy tree and have no children
        assert!(taffy.child_count(node) == 0);
    }

    /// Test that new_with_children works as expected
    #[test]
    fn test_new_with_children() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        // node should have two children
        assert_eq!(taffy.child_count(node), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child0);
        assert_eq!(taffy.children(node).unwrap()[1], child1);
    }

    #[test]
    fn remove_node_should_remove() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let node = taffy.new_leaf(Style::default()).unwrap();

        let _ = taffy.remove(node).unwrap();
    }

    #[test]
    fn remove_node_should_detach_hierarchy() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Build a linear tree layout: <0> <- <1> <- <2>
        let node2 = taffy.new_leaf(Style::default()).unwrap();
        let node1 = taffy.new_with_children(Style::default(), &[node2]).unwrap();
        let node0 = taffy.new_with_children(Style::default(), &[node1]).unwrap();

        // Both node0 and node1 should have 1 child nodes
        assert_eq!(taffy.children(node0).unwrap().as_slice(), &[node1]);
        assert_eq!(taffy.children(node1).unwrap().as_slice(), &[node2]);

        // Disconnect the tree: <0> <2>
        let _ = taffy.remove(node1).unwrap();

        // Both remaining nodes should have no child nodes
        assert!(taffy.children(node0).unwrap().is_empty());
        assert!(taffy.children(node2).unwrap().is_empty());
    }

    #[test]
    fn remove_last_node() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let parent = taffy.new_leaf(Style::default()).unwrap();
        let child = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(parent, child).unwrap();

        taffy.remove(child).unwrap();
        taffy.remove(parent).unwrap();
    }

    // Related to: https://github.com/DioxusLabs/taffy/issues/998
    #[test]
    fn remove_node_marks_parent_dirty() {
        use crate::prelude::*;

        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // The root's height comes entirely from its child's 1px bottom border
        let child = taffy
            .new_leaf(Style {
                border: Rect {
                    bottom: length(1f32),
                    ..Rect::zero()
                },
                ..Default::default()
            })
            .unwrap();
        let root = taffy.new_with_children(Style::default(), &[child]).unwrap();

        taffy.compute_layout(root, Size::MIN_CONTENT).unwrap();
        assert_eq!(taffy.layout(root).unwrap().size.height, 1f32);

        taffy.remove(child).unwrap();
        assert_eq!(taffy.dirty(root), Ok(true));

        // The root no longer has any children, so its height should shrink back to zero
        taffy.compute_layout(root, Size::MIN_CONTENT).unwrap();
        assert_eq!(taffy.layout(root).unwrap().size.height, 0f32);
    }

    #[test]
    fn set_measure() {
        let mut taffy: TaffyTree<Size<f32>> = TaffyTree::new();
        let node = taffy
            .new_leaf_with_context(
                Style::default(),
                Size {
                    width: 200.0,
                    height: 200.0,
                },
            )
            .unwrap();
        taffy
            .compute_layout_with_measure(node, Size::MAX_CONTENT, size_measure_function)
            .unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 200.0);

        taffy
            .set_node_context(
                node,
                Some(Size {
                    width: 100.0,
                    height: 100.0,
                }),
            )
            .unwrap();
        taffy
            .compute_layout_with_measure(node, Size::MAX_CONTENT, size_measure_function)
            .unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 100.0);
    }

    #[test]
    fn set_measure_of_previously_unmeasured_node() {
        let mut taffy: TaffyTree<Size<f32>> = TaffyTree::new();
        let node = taffy.new_leaf(Style::default()).unwrap();
        taffy
            .compute_layout_with_measure(node, Size::MAX_CONTENT, size_measure_function)
            .unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 0.0);

        taffy
            .set_node_context(
                node,
                Some(Size {
                    width: 100.0,
                    height: 100.0,
                }),
            )
            .unwrap();
        taffy
            .compute_layout_with_measure(node, Size::MAX_CONTENT, size_measure_function)
            .unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 100.0);
    }

    /// Test that adding `add_child()` works
    #[test]
    fn add_child() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let node = taffy.new_leaf(Style::default()).unwrap();
        assert_eq!(taffy.child_count(node), 0);

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(node, child0).unwrap();
        assert_eq!(taffy.child_count(node), 1);

        let child1 = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(node, child1).unwrap();
        assert_eq!(taffy.child_count(node), 2);
    }

    #[test]
    fn insert_child_at_index() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let child2 = taffy.new_leaf(Style::default()).unwrap();

        let node = taffy.new_leaf(Style::default()).unwrap();
        assert_eq!(taffy.child_count(node), 0);

        taffy.insert_child_at_index(node, 0, child0).unwrap();
        assert_eq!(taffy.child_count(node), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child0);

        taffy.insert_child_at_index(node, 0, child1).unwrap();
        assert_eq!(taffy.child_count(node), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child1);
        assert_eq!(taffy.children(node).unwrap()[1], child0);

        taffy.insert_child_at_index(node, 1, child2).unwrap();
        assert_eq!(taffy.child_count(node), 3);
        assert_eq!(taffy.children(node).unwrap()[0], child1);
        assert_eq!(taffy.children(node).unwrap()[1], child2);
        assert_eq!(taffy.children(node).unwrap()[2], child0);
    }

    #[test]
    fn set_children() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        assert_eq!(taffy.child_count(node), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child0);
        assert_eq!(taffy.children(node).unwrap()[1], child1);

        let child2 = taffy.new_leaf(Style::default()).unwrap();
        let child3 = taffy.new_leaf(Style::default()).unwrap();
        taffy.set_children(node, &[child2, child3]).unwrap();

        assert_eq!(taffy.child_count(node), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child2);
        assert_eq!(taffy.children(node).unwrap()[1], child3);
    }

    /// Test that removing a child works
    #[test]
    fn remove_child() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        assert_eq!(taffy.child_count(node), 2);

        taffy.remove_child(node, child0).unwrap();
        assert_eq!(taffy.child_count(node), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);

        taffy.remove_child(node, child1).unwrap();
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn remove_child_at_index() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        assert_eq!(taffy.child_count(node), 2);

        taffy.remove_child_at_index(node, 0).unwrap();
        assert_eq!(taffy.child_count(node), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);

        taffy.remove_child_at_index(node, 0).unwrap();
        assert_eq!(taffy.child_count(node), 0);
    }

    #[test]
    fn remove_children_range() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let child2 = taffy.new_leaf(Style::default()).unwrap();
        let child3 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1, child2, child3])
            .unwrap();

        assert_eq!(taffy.child_count(node), 4);

        taffy.remove_children_range(node, 1..=2).unwrap();
        assert_eq!(taffy.child_count(node), 2);
        assert_eq!(taffy.children(node).unwrap(), [child0, child3]);
        for child in [child0, child3] {
            assert_eq!(taffy.parent(child), Some(node));
        }
        for child in [child1, child2] {
            assert_eq!(taffy.parent(child), None);
        }
    }

    // Related to: https://github.com/DioxusLabs/taffy/issues/510
    #[test]
    fn remove_child_updates_parents() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let parent = taffy.new_leaf(Style::default()).unwrap();
        let child = taffy.new_leaf(Style::default()).unwrap();

        taffy.add_child(parent, child).unwrap();

        taffy.remove(parent).unwrap();

        // Once the parent is removed this shouldn't panic.
        assert!(taffy.set_children(child, &[]).is_ok());
    }

    #[test]
    fn replace_child_at_index() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();

        let node = taffy
            .new_with_children(Style::default(), &[child0])
            .unwrap();
        assert_eq!(taffy.child_count(node), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child0);

        taffy.replace_child_at_index(node, 0, child1).unwrap();
        assert_eq!(taffy.child_count(node), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);
    }
    #[test]
    fn test_child_at_index() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let child2 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1, child2])
            .unwrap();

        assert!(if let Ok(result) = taffy.child_at_index(node, 0) {
            result == child0
        } else {
            false
        });
        assert!(if let Ok(result) = taffy.child_at_index(node, 1) {
            result == child1
        } else {
            false
        });
        assert!(if let Ok(result) = taffy.child_at_index(node, 2) {
            result == child2
        } else {
            false
        });
    }
    #[test]
    fn test_child_count() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        assert!(taffy.child_count(node) == 2);
        assert!(taffy.child_count(child0) == 0);
        assert!(taffy.child_count(child1) == 0);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn test_children() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        let mut children = sys::Vec::new();
        children.push(child0);
        children.push(child1);

        let children_result = taffy.children(node).unwrap();
        assert_eq!(children_result, children);

        assert!(taffy.children(child0).unwrap().is_empty());
    }
    #[test]
    fn test_set_style() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let node = taffy.new_leaf(Style::default()).unwrap();
        assert_eq!(taffy.style(node).unwrap().display, Display::Flex);

        taffy
            .set_style(
                node,
                Style {
                    display: Display::None,
                    ..Style::default()
                },
            )
            .unwrap();
        assert_eq!(taffy.style(node).unwrap().display, Display::None);
    }
    #[test]
    fn test_style() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let style = Style {
            display: Display::None,
            flex_direction: FlexDirection::RowReverse,
            ..Default::default()
        };

        let node = taffy.new_leaf(style.clone()).unwrap();

        let res = taffy.style(node);
        assert!(res.is_ok());
        assert!(res.unwrap() == &style);
    }
    #[test]
    fn test_layout() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let node = taffy.new_leaf(Style::default()).unwrap();

        // TODO: Improve this test?
        let res = taffy.layout(node);
        assert!(res.is_ok());
    }

    #[test]
    fn test_mark_dirty() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy
            .new_with_children(Style::default(), &[child0, child1])
            .unwrap();

        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.dirty(child0), Ok(false));
        assert_eq!(taffy.dirty(child1), Ok(false));
        assert_eq!(taffy.dirty(node), Ok(false));

        taffy.mark_dirty(node).unwrap();
        assert_eq!(taffy.dirty(child0), Ok(false));
        assert_eq!(taffy.dirty(child1), Ok(false));
        assert_eq!(taffy.dirty(node), Ok(true));

        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        taffy.mark_dirty(child0).unwrap();
        assert_eq!(taffy.dirty(child0), Ok(true));
        assert_eq!(taffy.dirty(child1), Ok(false));
        assert_eq!(taffy.dirty(node), Ok(true));
    }

    #[test]
    #[cfg(feature = "std")]
    fn mark_dirty_twenty_thousand_ancestors_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut tree = TaffyTree::<()>::new();
                let leaf = tree.new_leaf(Style::default()).unwrap();
                let mut root = leaf;
                for _ in 0..20_000 {
                    root = tree.new_with_children(Style::default(), &[root]).unwrap();
                }
                let input = LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    ..LayoutInput::HIDDEN
                };
                for node in tree.nodes.values_mut() {
                    node.cache.store(&input, LayoutOutput::DEFAULT);
                }
                tree.mark_dirty(leaf).unwrap();
                assert!(tree.nodes.values().all(|node| node.cache.is_empty()));
                assert_eq!(tree.dirty(root), Ok(true));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[cfg(all(feature = "std", feature = "flexbox"))]
    fn percentage_height_twenty_thousand_forwarding_levels_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut tree = TaffyTree::<()>::new();
                let leaf = tree.new_leaf(Style::default()).unwrap();
                let forwarding: SharedStyle = Style {
                    percent_basis_from_parent: true,
                    ..Style::default()
                }
                .into();
                let mut root = leaf;
                for _ in 0..20_000 {
                    let sibling = tree.new_leaf(Style::default()).unwrap();
                    root = tree
                        .new_with_children(forwarding.clone(), &[root, sibling])
                        .unwrap();
                }
                assert!(!tree.row_uses_pct_height(root));
                tree.set_style(
                    leaf,
                    Style {
                        size: Size {
                            width: Dimension::AUTO,
                            height: Dimension::from_percent(0.5),
                        },
                        ..Style::default()
                    },
                )
                .unwrap();
                assert!(tree.row_uses_pct_height(root));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[cfg(feature = "flexbox")]
    fn percentage_height_scan_prunes_non_forwarding_children_but_checks_their_own_limits() {
        let mut tree = TaffyTree::<()>::new();
        let percentage = tree
            .new_leaf(Style {
                size: Size {
                    width: Dimension::AUTO,
                    height: Dimension::from_percent(0.5),
                },
                ..Style::default()
            })
            .unwrap();
        let boundary = tree
            .new_with_children(Style::default(), &[percentage])
            .unwrap();
        let root = tree
            .new_with_children(Style::default(), &[boundary])
            .unwrap();
        assert!(!tree.row_uses_pct_height(root));
        tree.set_style(
            boundary,
            Style {
                min_size: Size {
                    width: crate::style::LengthPercentageAuto::AUTO,
                    height: crate::style::LengthPercentageAuto::from_percent(0.25),
                },
                ..Style::default()
            },
        )
        .unwrap();
        assert!(tree.row_uses_pct_height(root));
    }

    fn round_tree(tree: &mut TaffyTree<()>, root: NodeId) {
        let mut view = TaffyView {
            taffy: tree,
            measure_function: crate::tree::SyncMeasure(|_, _, _: Option<&mut ()>, _: &Style| {
                LayoutOutput::DEFAULT
            }),
        };
        round_layout(&mut view, root);
    }

    #[test]
    #[cfg(feature = "std")]
    fn rounding_twenty_thousand_branched_levels_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut tree = TaffyTree::<()>::new();
                let leaf = tree.new_leaf(Style::default()).unwrap();
                let mut root = leaf;
                for _ in 0..20_000 {
                    let sibling = tree.new_leaf(Style::default()).unwrap();
                    root = tree
                        .new_with_children(Style::default(), &[root, sibling])
                        .unwrap();
                }
                for node in tree.nodes.values_mut() {
                    node.unrounded_layout.location = crate::geometry::Point { x: 0.25, y: -0.25 };
                    node.unrounded_layout.size = Size {
                        width: 40.5,
                        height: 20.5,
                    };
                }
                round_tree(&mut tree, root);
                assert!(tree.nodes.values().all(|node| node.final_layout.is_some()));
                assert_eq!(
                    tree.layout(leaf).unwrap().location,
                    crate::geometry::Point::ZERO
                );
                assert_eq!(
                    tree.layout(leaf).unwrap().size,
                    Size {
                        width: 41.0,
                        height: 20.0
                    }
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[cfg(feature = "std")]
    fn hidden_twenty_thousand_branched_levels_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut tree = TaffyTree::<()>::new();
                let mut root = tree.new_leaf(Style::default()).unwrap();
                for _ in 0..20_000 {
                    let sibling = tree.new_leaf(Style::default()).unwrap();
                    root = tree
                        .new_with_children(Style::default(), &[root, sibling])
                        .unwrap();
                }
                tree.set_style(
                    root,
                    Style {
                        display: Display::None,
                        ..Style::default()
                    },
                )
                .unwrap();
                let input = LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    ..LayoutInput::HIDDEN
                };
                for node in tree.nodes.values_mut() {
                    node.unrounded_layout.size = Size {
                        width: 40.5,
                        height: 20.5,
                    };
                    node.cache.store(&input, LayoutOutput::DEFAULT);
                }
                // Запускаем Display::None корня; потомки должны отбросить кэшированные результаты.
                tree.nodes[root.into()].cache.clear();
                tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |_, _, _, _| {
                    panic!("a hidden descendant must not be measured")
                })
                .unwrap();
                assert!(tree
                    .nodes
                    .values()
                    .all(|node| node.unrounded_layout.size == Size::ZERO));
                assert!(tree
                    .nodes
                    .values()
                    .all(|node| node.final_layout.as_ref().unwrap().size == Size::ZERO));
                assert!(tree
                    .nodes
                    .iter()
                    .all(|(key, node)| key == root.into() || node.cache.is_empty()));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn rounding_hoisted_descendants_uses_the_containing_block_origin() {
        use crate::geometry::Point;
        let mut tree = TaffyTree::<()>::new();
        let leaf = tree.new_leaf(Style::default()).unwrap();
        let absolute = tree
            .new_with_children(
                Style {
                    position: crate::style::Position::Absolute,
                    ..Style::default()
                },
                &[leaf],
            )
            .unwrap();
        let parent = tree
            .new_with_children(Style::default(), &[absolute])
            .unwrap();
        let sibling = tree.new_leaf(Style::default()).unwrap();
        let root = tree
            .new_with_children(Style::default(), &[parent, sibling])
            .unwrap();
        for (node, x) in [
            (root, 0.25),
            (parent, 10.25),
            (absolute, 1.25),
            (leaf, 0.25),
            (sibling, 3.25),
        ] {
            let layout = &mut tree.nodes[node.into()].unrounded_layout;
            layout.location = Point { x, y: x };
            layout.size = Size {
                width: 10.5,
                height: 10.5,
            };
        }
        tree.nodes[root.into()].hoisted_children.push(absolute);
        round_tree(&mut tree, root);
        assert_eq!(
            tree.layout(absolute).unwrap().location,
            Point { x: 2.0, y: 2.0 }
        );
        assert_eq!(tree.layout(leaf).unwrap().location, Point::ZERO);
        assert_eq!(
            tree.layout(leaf).unwrap().size,
            Size {
                width: 10.0,
                height: 10.0
            }
        );
        assert_eq!(
            tree.layout(sibling).unwrap().location,
            Point { x: 4.0, y: 4.0 }
        );
    }

    #[test]
    fn compute_layout_should_produce_valid_result() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let node_result = taffy.new_leaf(Style {
            size: Size {
                width: Dimension::from_length(10f32),
                height: Dimension::from_length(10f32),
            },
            ..Default::default()
        });
        assert!(node_result.is_ok());
        let node = node_result.unwrap();
        let layout_result = taffy.compute_layout(
            node,
            Size {
                width: AvailableSpace::Definite(100.),
                height: AvailableSpace::Definite(100.),
            },
        );
        assert!(layout_result.is_ok());
    }

    #[test]
    fn make_sure_layout_location_is_top_left() {
        use crate::prelude::*;

        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let node = taffy
            .new_leaf(Style {
                size: Size {
                    width: Dimension::from_percent(1f32),
                    height: Dimension::from_percent(1f32),
                },
                ..Default::default()
            })
            .unwrap();

        let root = taffy
            .new_with_children(
                Style {
                    size: Size {
                        width: Dimension::from_length(100f32),
                        height: Dimension::from_length(100f32),
                    },
                    padding: Rect {
                        left: length(10f32),
                        right: length(20f32),
                        top: length(30f32),
                        bottom: length(40f32),
                    },
                    ..Default::default()
                },
                &[node],
            )
            .unwrap();

        taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

        // If Layout::location represents top-left coord, 'node' location
        // must be (due applied 'root' padding): {x: 10, y: 30}.
        //
        // It's important, since result will be different for each other
        // coordinate space:
        // - bottom-left:  {x: 10, y: 40}
        // - top-right:    {x: 20, y: 30}
        // - bottom-right: {x: 20, y: 40}
        let layout = taffy.layout(node).unwrap();
        assert_eq!(layout.location.x, 10f32);
        assert_eq!(layout.location.y, 30f32);
    }

    #[test]
    fn set_children_reparents() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let child = taffy.new_leaf(Style::default()).unwrap();
        let old_parent = taffy.new_with_children(Style::default(), &[child]).unwrap();

        let new_parent = taffy.new_leaf(Style::default()).unwrap();
        taffy.set_children(new_parent, &[child]).unwrap();

        assert!(taffy.children(old_parent).unwrap().is_empty());
    }

    /// A context that increments a counter when dropped. Each test owns its own static
    /// counter so the two can run in parallel.
    struct DropCounted(&'static core::sync::atomic::AtomicUsize);

    impl Drop for DropCounted {
        fn drop(&mut self) {
            self.0.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn remove_drops_the_nodes_context() {
        static DROPS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
        let mut taffy: TaffyTree<DropCounted> = TaffyTree::new();
        let node = taffy
            .new_leaf_with_context(Style::DEFAULT, DropCounted(&DROPS))
            .unwrap();

        taffy.remove(node).unwrap();

        assert_eq!(
            DROPS.load(core::sync::atomic::Ordering::SeqCst),
            1,
            "removing a node must drop its context rather than strand it in node_context_data"
        );
    }

    #[test]
    fn clear_drops_every_nodes_context() {
        static DROPS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
        let mut taffy: TaffyTree<DropCounted> = TaffyTree::new();
        for _ in 0..8 {
            taffy
                .new_leaf_with_context(Style::DEFAULT, DropCounted(&DROPS))
                .unwrap();
        }

        taffy.clear();

        assert_eq!(
            DROPS.load(core::sync::atomic::Ordering::SeqCst),
            8,
            "clear() drops all nodes, so it must drop their contexts too"
        );
    }

    #[test]
    fn final_layout_storage_is_allocated_only_by_rounding() {
        let style = Style {
            size: Size::from_lengths(10.5, 10.5),
            ..Style::default()
        };
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        taffy.disable_rounding();
        let unrounded = taffy.new_leaf(style.clone()).unwrap();
        taffy.compute_layout(unrounded, Size::MAX_CONTENT).unwrap();
        assert!(taffy.nodes[unrounded.into()].final_layout.is_none());
        assert_eq!(taffy.layout(unrounded).unwrap().size.width, 10.5);

        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let rounded = taffy.new_leaf(style).unwrap();
        assert_eq!(taffy.layout(rounded).unwrap().size.width, 0.0);
        taffy.compute_layout(rounded, Size::MAX_CONTENT).unwrap();
        assert!(taffy.nodes[rounded.into()].final_layout.is_some());
        assert_eq!(taffy.layout(rounded).unwrap().size.width, 11.0);
    }

    #[test]
    fn nodes_share_a_style_until_one_is_replaced() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let shared = SharedStyle::new(Style {
            display: Display::Grid,
            ..Style::default()
        });
        let first = taffy.new_leaf(shared.clone()).unwrap();
        let second = taffy.new_with_children(shared.clone(), &[first]).unwrap();
        assert!(SharedStyle::ptr_eq(
            &taffy.shared_style(first).unwrap(),
            &shared
        ));
        assert!(SharedStyle::ptr_eq(
            &taffy.shared_style(second).unwrap(),
            &shared
        ));

        taffy
            .set_style(
                first,
                Style {
                    display: Display::Flex,
                    ..Style::default()
                },
            )
            .unwrap();
        assert_eq!(taffy.style(first).unwrap().display, Display::Flex);
        assert_eq!(taffy.style(second).unwrap().display, Display::Grid);
        assert_eq!(shared.display, Display::Grid);
    }

    #[test]
    fn adopting_an_equal_style_keeps_the_cache_and_a_different_one_is_refused() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let grid = || Style {
            display: Display::Grid,
            ..Style::default()
        };
        let node = taffy.new_leaf(grid()).unwrap();
        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        assert!(!taffy.dirty(node).unwrap());

        let equal = SharedStyle::new(grid());
        assert!(taffy.style_equals(node, &equal));
        assert!(taffy.adopt_equal_style(node, &equal));
        assert!(SharedStyle::ptr_eq(
            &taffy.shared_style(node).unwrap(),
            &equal
        ));
        assert!(!taffy.dirty(node).unwrap());

        let flex = SharedStyle::new(Style {
            display: Display::Flex,
            ..Style::default()
        });
        assert!(!taffy.style_equals(node, &flex));
        assert!(!taffy.adopt_equal_style(node, &flex));
        assert!(SharedStyle::ptr_eq(
            &taffy.shared_style(node).unwrap(),
            &equal
        ));
    }

    #[test]
    fn reset_node_takes_a_shared_style_and_starts_with_an_empty_cache() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let node = taffy.new_leaf(Style::default()).unwrap();
        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        let shared = SharedStyle::new(Style {
            display: Display::Grid,
            ..Style::default()
        });
        taffy.reset_node(node, shared.clone(), &[], None).unwrap();
        assert!(SharedStyle::ptr_eq(
            &taffy.shared_style(node).unwrap(),
            &shared
        ));
        assert!(taffy.dirty(node).unwrap());
    }

    #[test]
    fn reserve_nodes_grows_the_full_storage_once() {
        let mut taffy: TaffyTree<()> = TaffyTree::with_capacity(0);
        assert!(taffy.node_storage_full());
        taffy.reserve_nodes(1000);
        assert!(taffy.nodes.capacity() >= 1000);
        let capacity = taffy.nodes.capacity();
        let leaves: Vec<_> = (0..1000)
            .map(|_| taffy.new_leaf(Style::DEFAULT).unwrap())
            .collect();
        assert_eq!(
            taffy.nodes.capacity(),
            capacity,
            "no reallocation up to the hint"
        );
        let root = taffy.new_with_children(Style::DEFAULT, &leaves).unwrap();
        assert_eq!(taffy.child_count(root), 1000);
        // A smaller hint never shrinks or regrows the storage.
        taffy.reserve_nodes(10);
        assert!(taffy.nodes.capacity() >= 1001);
    }
}

#[cfg(all(
    test,
    feature = "std",
    feature = "block_layout",
    feature = "flexbox",
    feature = "grid"
))]
#[path = "layout_depth_tests.rs"]
mod layout_depth_tests;
