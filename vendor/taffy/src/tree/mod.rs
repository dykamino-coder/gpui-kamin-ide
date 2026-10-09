//! Contains both a high-level interface to Taffy using a ready-made node tree, and a set of traits for defining custom node trees.
//!
//! - For documentation on the high-level API, see the [`TaffyTree`] struct.
//! - For documentation on the low-level trait-based API, see the [`traits`] module.

// Submodules
mod cache;
mod layout;
mod node;
pub mod traits;

#[doc(hidden)]
#[cfg(all(debug_assertions, feature = "std"))]
pub use cache::cache_mode_change_evictions;
pub use cache::{Cache, ClearState};
pub use layout::{
    AxisStaticAlign, AxisStaticEdge, AxisStaticPosition, Baselines, CollapsibleMarginSet, Layout,
    LayoutInput, LayoutOutput, MeasureOutput, OofCandidate, OofCandidates, OofPositioningArea,
    RequestedAxis, RunMode, SizingMode,
};
pub use node::NodeId;
pub(crate) use traits::LayoutPartialTreeExt;
pub use traits::{
    LayoutContainingBlock, LayoutPartialTree, PrintTree, RoundTree, TraversePartialTree,
    TraverseTree,
};

#[cfg(feature = "flexbox")]
pub use traits::LayoutFlexboxContainer;

#[cfg(feature = "grid")]
pub use traits::LayoutGridContainer;

#[cfg(feature = "block_layout")]
pub use traits::LayoutBlockContainer;

#[cfg(feature = "taffy_tree")]
mod taffy_tree;
#[cfg(feature = "taffy_tree")]
pub use taffy_tree::{TaffyError, TaffyResult, TaffyTree};
#[cfg(feature = "taffy_tree")]
mod kamin_calc;
#[cfg(feature = "taffy_tree")]
mod kamin_output;
#[cfg(all(feature = "taffy_tree", feature = "std"))]
pub use kamin_calc::calc_handle;
#[cfg(feature = "taffy_tree")]
pub use kamin_calc::calc_value;

pub use layout::DetailedLayoutInfo;

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox"))]
mod flex_safe_alignment_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox"))]
mod scroll_baseline_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod grid_container_baseline_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox"))]
mod flex_wrap_limit_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_geometry_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_flow_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod subgrid_flow_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "block_layout"))]
mod block_flow_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "block_layout",
    feature = "float_layout"
))]
mod block_flow_float_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_constraint_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_stack_measure_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_intrinsic_keyword_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "flexbox",
    feature = "block_layout"
))]
mod ratio_preferred_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod lanes_intrinsic_baseline_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "grid",
    feature = "block_layout"
))]
mod lanes_min_content_fraction_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid", feature = "flexbox"))]
mod lanes_container_export_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod baseline_orientation_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "grid",
    feature = "block_layout"
))]
mod grid_x_baseline_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "grid",
    feature = "block_layout"
))]
mod lanes_x_baseline_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "flexbox",
    feature = "block_layout",
    feature = "grid"
))]
mod block_fit_content_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "flexbox",
    feature = "block_layout",
    feature = "grid"
))]
mod flex_x_baseline_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "flexbox",
    feature = "block_layout",
    feature = "grid"
))]
mod flex_x_groups_tests;

#[cfg(all(
    test,
    feature = "taffy_tree",
    feature = "flexbox",
    feature = "block_layout",
    feature = "grid"
))]
mod flex_x_relative_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox"))]
mod flex_intrinsic_cross_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "flexbox"))]
mod flex_fit_cross_tests;

#[cfg(all(test, feature = "taffy_tree"))]
mod leaf_xy_baselines_tests;

#[cfg(all(test, feature = "taffy_tree", feature = "grid"))]
mod grid_abspos_fit_tests;

#[cfg(test)]
mod lanes_absolute_flow_tests;

#[cfg(test)]
mod ratio_constraint_probe_tests;
