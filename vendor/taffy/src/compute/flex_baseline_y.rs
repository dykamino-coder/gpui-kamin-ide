//! Existing row/y baseline measurement, isolated from physical column/x groups.
use super::*;

/// Calculate the base lines of the children.
#[inline]
pub(super) fn calculate_children_base_lines(
    tree: &mut impl LayoutFlexboxContainer,
    node_size: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    flex_lines: &mut [FlexLine],
    constants: &AlgoConstants,
) {
    // Only compute baselines for flex rows because we only support baseline alignment in the cross axis
    // where that axis is also the inline axis
    // TODO: this may need revisiting if/when we support vertical writing modes
    if !constants.is_row {
        return;
    }

    for line in flex_lines {
        // If a flex line has one or zero items participating in baseline alignment then baseline alignment is a no-op so we skip
        let line_baseline_child_count = line
            .items
            .iter()
            .filter(|child| child.participates_in_baseline_alignment(constants.dir))
            .count();
        let last_count = line
            .items
            .iter()
            .filter(|child| child.participates_in_last_baseline_alignment(constants.dir))
            .count();
        if line_baseline_child_count <= 1 && last_count <= 1 {
            continue;
        }

        for child in line.items.iter_mut() {
            // Only calculate baselines for children participating in baseline alignment
            let is_first = child.participates_in_baseline_alignment(constants.dir)
                && line_baseline_child_count > 1;
            let is_last =
                child.participates_in_last_baseline_alignment(constants.dir) && last_count > 1;
            if !is_first && !is_last {
                continue;
            }

            let measured_size_and_baselines = tree.compute_child_layout(
                child.node,
                LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    sizing_mode: SizingMode::ContentSize,
                    axis: RequestedAxis::Both,
                    known_dimensions: Size {
                        width: if constants.is_row {
                            child.target_size.width.into()
                        } else {
                            child.hypothetical_inner_size.width.into()
                        },
                        height: if constants.is_row {
                            child.hypothetical_inner_size.height.into()
                        } else {
                            child.target_size.height.into()
                        },
                    },
                    known_dimensions_are_definite: item_known_dimension_definiteness(
                        constants, child,
                    ),
                    parent_size: constants.pct_basis(),
                    available_space: Size {
                        width: if constants.is_row {
                            constants.container_size.width.into()
                        } else {
                            available_space.width.maybe_set(node_size.width)
                        },
                        height: if constants.is_row {
                            available_space.height.maybe_set(node_size.height)
                        } else {
                            constants.container_size.height.into()
                        },
                    },
                    vertical_margins_are_collapsible: Line::FALSE,
                },
            );

            let baseline = measured_size_and_baselines.baselines.first;
            let height = measured_size_and_baselines.size.height;

            if is_last {
                child.last_baseline_from_end = measured_size_and_baselines
                    .last_or_first_y()
                    .map_or(0.0, |baseline| {
                        let baseline = if child.overflow.y.is_scroll_container() {
                            baseline.min(height).max(0.0)
                        } else {
                            baseline
                        };
                        height - baseline + child.margin.bottom
                    });
                continue;
            }

            // Scroll containers' baselines are determined from their content as if scrolled to the
            // initial position, but are additionally clamped to their border box.
            // See https://github.com/w3c/csswg-drafts/issues/7660
            let baseline = baseline
                .map(|baseline| {
                    if child.overflow.y.is_scroll_container() {
                        baseline.min(height).max(0.0)
                    } else {
                        baseline
                    }
                })
                .unwrap_or(height + child.margin.bottom);

            child.baseline = baseline + child.margin.top;
        }
    }
}
