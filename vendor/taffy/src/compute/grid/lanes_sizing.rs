//! Resolve authored lane-item sizing keywords through native Taffy measurement.
//! Linked subgrid axes must be masked to auto by the caller before entry.
use crate::compute::common::sizing_keyword::{resolve_sizing_keyword, SizingKeywordResolution};
use crate::geometry::{Line, Size};
use crate::style::{AvailableSpace, Dimension};
use crate::tree::{
    LayoutInput, LayoutPartialTree, LayoutPartialTreeExt, NodeId, RequestedAxis, RunMode,
    SizingMode,
};

pub(super) fn resolve_lane_sizing_keywords(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    styles: Size<Dimension>,
    parent: Size<Option<f32>>,
    stretch: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    known: Size<Option<f32>>,
) -> Size<Option<f32>> {
    let width = if known.width.is_none() {
        resolve_sizing_keyword(styles.width, stretch.width, parent.width, |v, b| {
            tree.calc(v, b)
        })
    } else {
        None
    };
    let height = if known.height.is_none() {
        resolve_sizing_keyword(styles.height, stretch.height, parent.height, |v, b| {
            tree.calc(v, b)
        })
    } else {
        None
    };
    let exact = Size {
        width: match width {
            Some(SizingKeywordResolution::Exact(v)) => Some(v),
            _ => None,
        },
        height: match height {
            Some(SizingKeywordResolution::Exact(v)) => Some(v),
            _ => None,
        },
    };
    let measure_width = match width {
        Some(SizingKeywordResolution::Measure(v)) => Some(v),
        _ => None,
    };
    let measure_height = match height {
        Some(SizingKeywordResolution::Measure(v)) => Some(v),
        _ => None,
    };
    if measure_width.is_none() && measure_height.is_none() {
        return exact;
    }
    // Use one joint probe for two keywords, retaining the other authored/exact axis.
    let probe_known = known.or(exact);
    let measured = tree
        .compute_child_layout(
            node,
            LayoutInput {
                known_dimensions: probe_known,
                known_dimensions_are_definite: Size {
                    width: true,
                    height: true,
                },
                parent_size: parent,
                available_space: Size {
                    width: measure_width.unwrap_or(available.width),
                    height: measure_height.unwrap_or(available.height),
                },
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                run_mode: RunMode::ComputeSize,
                vertical_margins_are_collapsible: Line::FALSE,
            },
        )
        .size;
    Size {
        width: exact
            .width
            .or_else(|| measure_width.map(|_| measured.width)),
        height: exact
            .height
            .or_else(|| measure_height.map(|_| measured.height)),
    }
}

#[cfg(all(test, feature = "taffy_tree"))]
mod tests {
    use super::*;
    use crate::{LengthPercentage, Rect, Style, TaffyTree};
    fn resolve(
        styles: Size<Dimension>,
        parent: Size<Option<f32>>,
        stretch: Size<Option<f32>>,
        known: Size<Option<f32>>,
    ) -> Size<Option<f32>> {
        let mut tree: TaffyTree = TaffyTree::new();
        // An unexpected probe would fail because NodeId(0) does not exist.
        let result = resolve_lane_sizing_keywords(
            &mut tree.as_layout_tree(),
            NodeId::new(0),
            styles,
            parent,
            stretch,
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
            known,
        );
        result
    }
    #[test]
    fn masked_linked_axes_do_not_trigger_keyword_measurement() {
        assert_eq!(
            resolve(
                Size {
                    width: Dimension::auto(),
                    height: Dimension::auto()
                },
                Size::NONE,
                Size::NONE,
                Size::NONE
            ),
            Size::NONE
        );
    }
    #[test]
    fn percentage_fit_content_with_unknown_basis_behaves_as_auto() {
        assert_eq!(
            resolve(
                Size {
                    width: Dimension::fit_content_percent(0.5),
                    height: Dimension::auto()
                },
                Size::NONE,
                Size::NONE,
                Size::NONE
            ),
            Size::NONE
        );
    }
    #[test]
    fn stretch_uses_the_margin_reduced_area_without_a_probe() {
        assert_eq!(
            resolve(
                Size {
                    width: Dimension::stretch(),
                    height: Dimension::auto()
                },
                Size {
                    width: Some(100.0),
                    height: None
                },
                Size {
                    width: Some(80.0),
                    height: None
                },
                Size::NONE
            ),
            Size {
                width: Some(80.0),
                height: None
            }
        );
    }
    #[test]
    fn already_known_axis_takes_precedence() {
        assert_eq!(
            resolve(
                Size {
                    width: Dimension::min_content(),
                    height: Dimension::auto()
                },
                Size::NONE,
                Size::NONE,
                Size {
                    width: Some(25.0),
                    height: None
                }
            ),
            Size::NONE
        );
    }
    #[test]
    fn both_measured_keywords_keep_native_border_box_size() {
        let mut tree: TaffyTree = TaffyTree::new();
        let node = tree
            .new_leaf(Style {
                padding: Rect {
                    left: LengthPercentage::length(4.0),
                    right: LengthPercentage::length(6.0),
                    top: LengthPercentage::length(3.0),
                    bottom: LengthPercentage::length(7.0),
                },
                ..Style::default()
            })
            .unwrap();
        let resolved = resolve_lane_sizing_keywords(
            &mut tree.as_layout_tree(),
            node,
            Size {
                width: Dimension::min_content(),
                height: Dimension::max_content(),
            },
            Size::NONE,
            Size::NONE,
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
            Size::NONE,
        );
        assert_eq!(
            resolved,
            Size {
                width: Some(10.0),
                height: Some(10.0)
            }
        );
    }
}
