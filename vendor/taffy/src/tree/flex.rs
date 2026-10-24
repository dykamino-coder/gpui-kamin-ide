//! KaminIDE patch: финальная геометрия flex для потребителей раскладки (css-flexbox-1 §10).

use crate::style::FlexDirection;
use crate::tree::NodeId;
use crate::util::sys::Vec;

/// Flex lines from the final layout pass, in physical placement traversal order.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailedFlexInfo {
    /// Selects the physical main/cross axes; offsets increase rightwards/downwards,
    /// including for reverse directions and RTL. Relative positioning is excluded.
    pub direction: FlexDirection,
    /// Actual solver lines, including empty lines when produced by the algorithm.
    pub lines: Vec<DetailedFlexLine>,
}

/// One solver line, after cross-axis sizing and align-content distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailedFlexLine {
    /// Physical cross-axis start relative to the container border box.
    pub cross_offset: f32,
    /// Used cross size, excluding the gap to the next line.
    pub cross_size: f32,
    /// In-flow items placed on this line; hidden and out-of-flow children are absent.
    pub items: Vec<DetailedFlexItem>,
}

/// One item's final normal-flow border box in the container's physical axes.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailedFlexItem {
    /// Stable identity, independent of hidden siblings or flex ordering.
    pub node: NodeId,
    /// Child index used by the final Layout, before reversing placement traversal.
    pub order: u32,
    /// Normal-flow border-box start on the main axis.
    pub main_offset: f32,
    /// Normal-flow border-box start on the cross axis.
    pub cross_offset: f32,
    /// Final border-box main size, including any child layout adjustment.
    pub main_size: f32,
    /// Final border-box cross size, including any child layout adjustment.
    pub cross_size: f32,
}
