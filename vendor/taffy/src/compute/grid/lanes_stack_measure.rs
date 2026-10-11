//! Resolve a row lane's stacking width using its final physical inline track.
use crate::geometry::{Line, Size};
use crate::style::AvailableSpace;
use crate::tree::{LayoutGridContainer, LayoutInput, LayoutPartialTreeExt, NodeId, SizingMode};
use crate::{AbsoluteAxis, BlockFlow, CompactLength, CoreStyle, Dimension, ResolveOrZero};

/// A parent's intrinsic probe does not replace an authored intrinsic size.
pub(super) fn container_space(
    styles: Size<Dimension>,
    flow: Option<BlockFlow>,
    input: LayoutInput,
) -> Size<AvailableSpace> {
    if input.sizing_mode != SizingMode::InherentSize {
        return input.available_space;
    }
    let vertical = flow.is_some_and(|flow| flow.vertical);
    let resolve = |style: Dimension, known: Option<f32>, available, block_axis: bool| {
        if known.is_some() {
            return available;
        }
        match style.tag() {
            // CSS Sizing 3: authored min-content block size behaves as auto.
            // Keep intrinsic parent probes and inline-axis min-content distinct.
            CompactLength::MIN_CONTENT_TAG if block_axis => AvailableSpace::MaxContent,
            CompactLength::MIN_CONTENT_TAG => AvailableSpace::MinContent,
            CompactLength::MAX_CONTENT_TAG => AvailableSpace::MaxContent,
            _ => available,
        }
    };
    Size {
        width: resolve(
            styles.width,
            input.known_dimensions.width,
            input.available_space.width,
            vertical,
        ),
        height: resolve(
            styles.height,
            input.known_dimensions.height,
            input.available_space.height,
            !vertical,
        ),
    }
}

pub(super) fn width(
    tree: &mut impl LayoutGridContainer,
    node: NodeId,
    height: Option<f32>,
    parent: Size<Option<f32>>,
    area: f32,
) -> f32 {
    // Upstream #1234: the parent passes the border-box space, so the child's own vertical
    // margins are deducted here (auto margins count as zero).
    let margin = tree
        .get_core_container_style(node)
        .margin()
        .resolve_or_zero(parent.width, |val, basis| tree.calc(val, basis));
    let area = (area - margin.vertical_axis_sum()).max(0.0);
    tree.measure_child_size(
        node,
        Size {
            width: None,
            height,
        },
        parent,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::Definite(area),
        },
        SizingMode::InherentSize,
        AbsoluteAxis::Horizontal,
        Line::FALSE,
    )
}

#[cfg(test)]
#[path = "block_intrinsic_keyword_tests.rs"]
mod block_intrinsic_keyword_tests;
