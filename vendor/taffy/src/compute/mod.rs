//! Low-level access to the layout algorithms themselves. For a higher-level API, see the [`TaffyTree`](crate::TaffyTree) struct.
//!
//! ### Layout functions
//!
//! The layout functions all take an [`&mut impl LayoutPartialTree`](crate::LayoutPartialTree) parameter, which represents a single container node and it's direct children.
//!
//! | Function                          | Purpose                                                                                                                                                                                            |
//! | ---                               | ---                                                                                                                                                                                                |
//! | [`compute_flexbox_layout`]        | Layout a Flexbox container and it's direct children                                                                                                                                                |
//! | [`compute_grid_layout`]           | Layout a CSS Grid container and it's direct children                                                                                                                                               |
//! | [`compute_block_layout`]          | Layout a Block container and it's direct children                                                                                                                                                  |
//! | [`compute_leaf_layout`]           | Applies common properties like padding/border/aspect-ratio to a node before deferring to a passed closure to determine it's size. Can be applied to nodes like text or image nodes.                |
//! | [`compute_root_layout`]           | Layout the root node of a tree (regardless of it's layout mode). This function is typically called once to begin a layout run.                                                                     |                                                                      |
//! | [`compute_hidden_layout`]         | Mark a node as hidden during layout (like `Display::None`)                                                                                                                                         |
//! | [`compute_cached_layout`]         | Attempts to find a cached layout for the specified node and layout inputs. Uses the provided closure to compute the layout (and then stores the result in the cache) if no cached layout is found. |
//!
//! ### Other functions
//!
//! | Function                          | Requires                                                                                                                                                                                           | Purpose                                                              |
//! | ---                               | ---                                                                                                                                                                                                | ---                                                                  |
//! | [`round_layout`]                  | [`RoundTree`]                                                                                                                                                                                      | Round a tree of float-valued layouts to integer pixels               |
//! | [`print_tree`](crate::print_tree) | [`PrintTree`](crate::PrintTree)                                                                                                                                                                    | Print a debug representation of a node tree and it's computed layout |
//!
pub(crate) mod common;
// KaminIDE patch: all three container algorithms use the same measurement limits.
#[cfg(any(feature = "block_layout", feature = "flexbox", feature = "grid"))]
mod content_size_limits;
pub(crate) mod leaf;
pub(crate) mod oof;
mod round;
pub use round::round_layout;

#[cfg(feature = "block_layout")]
pub(crate) mod block;
#[cfg(feature = "block_layout")]
mod block_flow;
#[cfg(feature = "block_layout")]
mod root_block_constraints;

#[cfg(feature = "float_layout")]
pub(crate) mod float;

#[cfg(feature = "flexbox")]
pub(crate) mod flexbox;

#[cfg(feature = "grid")]
pub(crate) mod grid;

pub use leaf::{compute_leaf_layout, compute_leaf_layout_async};
pub use oof::{
    compute_oof_layout_async, compute_oof_layout_for_area_async, resolve_static_offset,
    OofLayoutResult,
};

#[cfg(feature = "block_layout")]
pub use self::block::{
    compute_block_align_content_offset, compute_block_layout_async, BlockContext,
    BlockFormattingContext,
};

#[cfg(feature = "flexbox")]
pub use self::flexbox::compute_flexbox_layout_async;

#[cfg(feature = "grid")]
pub use self::grid::compute_grid_layout_async;

#[cfg(feature = "float_layout")]
pub use self::block::FloatMark;
#[cfg(feature = "float_layout")]
pub use self::float::{BfcSlot, ContentSlot, FloatContext, FloatIntrinsicWidthCalculator};

use crate::geometry::{Line, Point, Size};
use crate::style::{AvailableSpace, ContainingBlockClaims, CoreStyle, Overflow};
use crate::tree::{
    Layout, LayoutInput, LayoutOutput, LayoutPartialTree, LayoutPartialTreeExt, NodeId,
    OofCandidates, RequestedAxis, RunMode, SizingMode,
};
use crate::util::debug::{debug_log, debug_log_node, debug_pop_node, debug_push_node};
use crate::util::sys::Vec;
use crate::util::ResolveOrZero;
use crate::CacheTree;

/// Compute layout for the root node in the tree
///
/// KaminIDE patch: GPUI lays out arbitrary element subtrees as layout roots (`layout_as_root*`
/// measures intrinsic contributions and fragments), so the root keeps the pre-#1204/#1205 semantics:
/// it is laid out in flow against `available_space` (margins are not subtracted), its border box is
/// placed at the origin (or the physical end edge for reversed flows), and its `position`/`inset`
/// styles are ignored. Out-of-flow candidates no ancestor claimed are still laid out against the
/// initial containing block by the final positioning pass below.
pub async fn compute_root_layout_async(
    tree: &mut (impl crate::tree::LayoutContainingBlock + CacheTree),
    root: NodeId,
    available_space: Size<AvailableSpace>,
    stack: &mut reblessive::Stk,
) {
    let direction = tree.get_core_container_style(root).direction();
    let icb_size = available_space.into_options();

    // When the root's layout is served from the cache its layout algorithm does not run, so the
    // hoisted children it recorded on a previous run (including those added by the root
    // positioning pass below) are still in place and must not be re-added.
    let mut root_is_cached = false;

    let (layout, candidates) =
        compute_in_flow_root_layout_async(tree, root, available_space, &mut root_is_cached, stack)
            .await;

    // Final positioning pass for out-of-flow boxes with no nearer containing block: the initial
    // containing block is the containing block for `position: fixed` boxes and for
    // `position: absolute` boxes with no positioned ancestor.
    if !candidates.is_empty() {
        let overflow = tree.get_core_container_style(root).overflow();

        // The initial containing block has the dimensions of the viewport (the available space) and is
        // anchored at the canvas origin. In an axis where the available space is indefinite, fall back
        // to the root's padding box.
        let area_inset = layout.border
            + crate::geometry::Rect {
                left: 0.0,
                right: layout.scrollbar_size.width,
                top: 0.0,
                bottom: layout.scrollbar_size.height,
            };
        let root_padding_box_size = layout.size
            - Size {
                width: area_inset.horizontal_axis_sum(),
                height: area_inset.vertical_axis_sum(),
            };
        let (area_width, area_x) = match icb_size.width {
            Some(width) => (
                (width - layout.scrollbar_size.width).max(0.0),
                -layout.location.x,
            ),
            None => (root_padding_box_size.width, area_inset.left),
        };
        let (area_height, area_y) = match icb_size.height {
            Some(height) => (
                (height - layout.scrollbar_size.height).max(0.0),
                -layout.location.y,
            ),
            None => (root_padding_box_size.height, area_inset.top),
        };
        let area_size = Size {
            width: area_width,
            height: area_height,
        };
        let area_offset = Point {
            x: area_x,
            y: area_y,
        };

        let mut hoisted: Vec<NodeId> = Vec::new();
        let mut unclaimed = OofCandidates::new();
        oof::perform_oof_layout_async(
            tree,
            root,
            candidates.as_slice(),
            area_size,
            area_offset,
            direction,
            // The root is the initial containing block and claims all remaining candidates
            ContainingBlockClaims::ALL,
            overflow,
            &mut hoisted,
            &mut unclaimed,
            stack,
        )
        .await;
        debug_assert!(
            unclaimed.is_empty(),
            "the root positioning pass must claim all remaining candidates"
        );
        if !root_is_cached {
            tree.add_hoisted_children(root, &hoisted);
        }
    }
}

/// Lay out an in-flow (or `position: relative`) root node against the available space, store its layout
/// and return it along with the out-of-flow candidates bubbled up from its subtree.
#[inline(always)]
async fn compute_in_flow_root_layout_async(
    tree: &mut (impl crate::tree::LayoutContainingBlock + CacheTree),
    root: NodeId,
    available_space: Size<AvailableSpace>,
    root_is_cached: &mut bool,
    stack: &mut reblessive::Stk,
) -> (Layout, OofCandidates) {
    #[cfg(feature = "block_layout")]
    let known_dimensions = root_block_constraints::known(tree, root, available_space);
    #[cfg(not(feature = "block_layout"))]
    let known_dimensions = Size::NONE;

    let inputs = LayoutInput {
        known_dimensions,
        known_dimensions_are_definite: Size {
            width: true,
            height: true,
        },
        parent_size: available_space.into_options(),
        available_space,
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        run_mode: RunMode::PerformLayout,
        vertical_margins_are_collapsible: Line::FALSE,
    };
    *root_is_cached = tree.cache_get(root, &inputs).is_some();

    // Recursively compute node layout
    let mut output = tree.compute_child_layout_async(root, inputs, stack).await;
    let style = tree.get_core_container_style(root);
    #[cfg(feature = "block_layout")]
    let flow = style.block_flow();
    #[cfg(feature = "block_layout")]
    let inline_space = if flow.is_some_and(|flow| flow.vertical) {
        available_space.height
    } else {
        available_space.width
    };
    #[cfg(not(feature = "block_layout"))]
    let inline_space = available_space.width;
    let padding = style
        .padding()
        .resolve_or_zero(inline_space.into_option(), |val, basis| {
            tree.calc(val, basis)
        });
    let border = style
        .border()
        .resolve_or_zero(inline_space.into_option(), |val, basis| {
            tree.calc(val, basis)
        });
    let margin = style
        .margin()
        .resolve_or_zero(inline_space.into_option(), |val, basis| {
            tree.calc(val, basis)
        });
    let scrollbar_size = Size {
        width: if style.overflow().y == Overflow::Scroll {
            style.scrollbar_width()
        } else {
            0.0
        },
        height: if style.overflow().x == Overflow::Scroll {
            style.scrollbar_width()
        } else {
            0.0
        },
    };
    #[cfg(feature = "block_layout")]
    let reversed = flow.map_or(
        Size {
            width: style.direction().is_rtl(),
            height: false,
        },
        |flow| Size {
            width: if flow.vertical {
                flow.block_reverse
            } else {
                flow.inline_reverse
            },
            height: flow.vertical && flow.inline_reverse,
        },
    );
    #[cfg(not(feature = "block_layout"))]
    let reversed = Size {
        width: style.direction().is_rtl(),
        height: false,
    };
    let location = Point {
        x: if reversed.width {
            available_space
                .width
                .into_option()
                .map_or(0.0, |available_width| available_width - output.size.width)
        } else {
            0.0
        },
        y: if reversed.height {
            available_space
                .height
                .into_option()
                .map_or(0.0, |height| height - output.size.height)
        } else {
            0.0
        },
    };

    drop(style);

    let layout = Layout {
        order: 0,
        location,
        relative_offset: Point::ZERO,
        size: output.size,
        #[cfg(feature = "content_size")]
        scrollable_overflow_rect: output.scrollable_overflow_rect,
        scrollbar_size,
        padding,
        border,
        // TODO: support auto margins for root node?
        margin,
    };
    tree.set_unrounded_layout(root, &layout);

    (layout, output.oof_candidates.take())
}

/// Attempts to find a cached layout for the specified node and layout inputs.
///
/// Uses the provided closure to compute the layout (and then stores the result in the cache) if no cached layout is found.
#[inline(always)]
pub fn compute_cached_layout<Tree: CacheTree + ?Sized, ComputeFunction>(
    tree: &mut Tree,
    node: NodeId,
    inputs: LayoutInput,
    compute_uncached: ComputeFunction,
) -> LayoutOutput
where
    ComputeFunction: FnOnce(&mut Tree, NodeId, LayoutInput) -> LayoutOutput,
{
    debug_push_node!(node);

    // First we check if we have a cached result for the given input
    let cache_entry = tree.cache_get(node, &inputs);
    if let Some(cached_size_and_baselines) = cache_entry {
        debug_log_node!(inputs);
        debug_log!("RESULT (CACHED)", dbg:cached_size_and_baselines.size);
        debug_pop_node!();
        return cached_size_and_baselines;
    }

    debug_log_node!(inputs);

    let computed_size_and_baselines = compute_uncached(tree, node, inputs);

    // Cache result
    store_cached_layout(tree, node, &inputs, &computed_size_and_baselines);

    debug_log!("RESULT", dbg:computed_size_and_baselines.size);
    debug_pop_node!();

    computed_size_and_baselines
}

// KaminIDE patch: cloning/moving the large output is a completion operation,
// separate from the recursive cache-miss frame.
/// Stores a completed layout result in the node cache.
#[inline(never)]
fn store_cached_layout<Tree: CacheTree + ?Sized>(
    tree: &mut Tree,
    node: NodeId,
    inputs: &LayoutInput,
    output: &LayoutOutput,
) {
    tree.cache_store(node, inputs, output.clone());
}

/// Creates a layout for this node and its children, recursively.
/// Each hidden node has zero size and is placed at the origin
pub async fn compute_hidden_layout_async(
    tree: &mut (impl LayoutPartialTree + CacheTree),
    node: NodeId,
    stack: &mut reblessive::Stk,
) -> LayoutOutput {
    // Clear cache and set zeroed-out layout for the node
    tree.cache_clear(node);
    tree.set_unrounded_layout(node, &Layout::with_order(0));

    // Perform hidden layout on all children
    for index in 0..tree.child_count(node) {
        let child_id = tree.get_child_id(node, index);
        tree.compute_child_layout_async(child_id, LayoutInput::HIDDEN, stack)
            .await;
    }

    LayoutOutput::HIDDEN
}

/// A module for unified re-exports of detailed layout info structs, used by low level API
pub mod detailed_info {
    #[cfg(feature = "grid")]
    pub use super::grid::{
        DetailedGridInfo, DetailedGridItemsInfo, DetailedGridTracksInfo, GridLineNames,
        GridLineNamesIter,
    };
}

#[cfg(test)]
mod tests {
    use super::compute_hidden_layout;
    use crate::geometry::{Point, Size};
    use crate::style::{Display, Style};
    use crate::TaffyTree;

    #[test]
    fn hidden_layout_should_hide_recursively() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let style: Style = Style {
            display: Display::Flex,
            size: Size::from_lengths(50.0, 50.0),
            ..Default::default()
        };

        let grandchild_00 = taffy.new_leaf(style.clone()).unwrap();
        let grandchild_01 = taffy.new_leaf(style.clone()).unwrap();
        let child_00 = taffy
            .new_with_children(style.clone(), &[grandchild_00, grandchild_01])
            .unwrap();

        let grandchild_02 = taffy.new_leaf(style.clone()).unwrap();
        let child_01 = taffy
            .new_with_children(style.clone(), &[grandchild_02])
            .unwrap();

        let root = taffy
            .new_with_children(
                Style {
                    display: Display::None,
                    size: Size::from_lengths(50.0, 50.0),
                    ..Default::default()
                },
                &[child_00, child_01],
            )
            .unwrap();

        compute_hidden_layout(&mut taffy.as_layout_tree(), root);

        // Whatever size and display-mode the nodes had previously,
        // all layouts should resolve to ZERO due to the root's DISPLAY::NONE

        for node in [
            root,
            child_00,
            child_01,
            grandchild_00,
            grandchild_01,
            grandchild_02,
        ] {
            let layout = taffy.layout(node).unwrap();
            assert_eq!(layout.size, Size::zero());
            assert_eq!(layout.location, Point::zero());
        }
    }
}

mod sync_api;
pub use sync_api::*;
mod stack_cache;
pub(crate) use stack_cache::compute_cached_layout_async;
