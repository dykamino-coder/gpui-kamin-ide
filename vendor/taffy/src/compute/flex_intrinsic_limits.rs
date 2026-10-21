//! Explicit intrinsic flex limits are measured independently of the flex basis.
use super::{AlgoConstants, FlexItem};
use crate::geometry::{AbsoluteAxis, Line, Size};
use crate::style::{AlignItemsKeyword, AvailableSpace};
use crate::tree::{LayoutFlexboxContainer, LayoutPartialTreeExt, SizingMode};
use crate::util::MaybeMath;
use crate::FlexboxItemStyle;

/// KaminIDE patch: Resolve intrinsic flex-item min/max constraints.
pub(super) fn resolve(
    tree: &mut impl LayoutFlexboxContainer,
    constants: &AlgoConstants,
    items: &mut [FlexItem],
) {
    for item in items {
        let style = tree.get_flexbox_child_style(item.node);
        let min = style.min_size_keywords();
        let max = style.max_size_keywords();
        drop(style);
        if min.width.is_none()
            && min.height.is_none()
            && max.width.is_none()
            && max.height.is_none()
        {
            continue;
        }
        // Probes return border-box intrinsic sizes, including the item's edges.
        // The measured axis has no known preferred size and ignores its limits.
        for (space, minimum) in [(min.width, true), (max.width, false)] {
            if let Some(space) = space {
                let width = tree.measure_child_size(
                    item.node,
                    Size {
                        width: None,
                        height: item.size.height,
                    },
                    constants.node_inner_size,
                    Size {
                        width: space,
                        height: constants
                            .node_inner_size
                            .height
                            .map(AvailableSpace::Definite)
                            .unwrap_or(AvailableSpace::MaxContent),
                    },
                    SizingMode::IntrinsicSize,
                    AbsoluteAxis::Horizontal,
                    Line::FALSE,
                );
                if minimum {
                    item.min_size.width = Some(width);
                } else {
                    item.max_size.width = Some(width);
                }
            }
        }
        let stretched_width = (constants.is_column
            && item.align_self.keyword == AlignItemsKeyword::Stretch
            && !item.margin_is_auto.left
            && !item.margin_is_auto.right)
            .then(|| {
                constants
                    .node_inner_size
                    .width
                    .maybe_sub(item.margin.horizontal_axis_sum())
            })
            .flatten();
        let known_width = item.size.width.or(stretched_width).map(|w| {
            w.maybe_min(item.max_size.width)
                .maybe_max(item.min_size.width)
        });
        for (space, minimum) in [(min.height, true), (max.height, false)] {
            if space.is_some() {
                // In the item's block axis, both intrinsic keywords use the
                // content height at the resolved inline size (CSS Sizing §3.2).
                let height = tree.measure_child_size(
                    item.node,
                    Size {
                        width: known_width,
                        height: None,
                    },
                    constants.node_inner_size,
                    Size {
                        width: known_width
                            .or(constants.node_inner_size.width)
                            .map(AvailableSpace::Definite)
                            .unwrap_or(AvailableSpace::MaxContent),
                        height: AvailableSpace::MaxContent,
                    },
                    SizingMode::IntrinsicSize,
                    AbsoluteAxis::Vertical,
                    Line::FALSE,
                );
                if minimum {
                    item.min_size.height = Some(height);
                } else {
                    item.max_size.height = Some(height);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tree/flex_intrinsic_limits_tests.rs"]
mod tests;
