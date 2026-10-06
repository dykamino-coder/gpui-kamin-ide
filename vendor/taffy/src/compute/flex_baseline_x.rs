//! Physical x baselines for column flex lines, including intrinsic cross size.
//! CSS Align 3 §9.2: opposite flow with opposite preference shares the same side.
use super::*;

fn side(child: &FlexItem, constants: &AlgoConstants) -> Option<bool> {
    let last = child.participates_in_last_baseline_alignment(constants.dir);
    if !last && !child.participates_in_baseline_alignment(constants.dir) {
        return None;
    }
    Some((child.baseline_x_flags & 1 != 0) != last)
}

pub(super) fn measure(
    tree: &mut impl LayoutFlexboxContainer,
    node_size: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    lines: &mut [FlexLine],
    constants: &AlgoConstants,
) {
    for line in lines {
        let counts = [false, true].map(|end| {
            line.items
                .iter()
                .filter(|child| side(child, constants) == Some(end))
                .count()
        });
        for child in line.items.iter_mut() {
            let Some(end) = side(child, constants) else {
                continue;
            };
            if counts[usize::from(end)] < 2 {
                continue;
            }
            let output = tree.compute_child_layout(
                child.node,
                LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    sizing_mode: SizingMode::ContentSize,
                    axis: RequestedAxis::Both,
                    known_dimensions: Size {
                        width: Some(child.hypothetical_inner_size.width),
                        height: Some(child.target_size.height),
                    },
                    known_dimensions_are_definite: item_known_dimension_definiteness(
                        constants, child,
                    ),
                    parent_size: constants.pct_basis(),
                    available_space: Size {
                        width: available.width.maybe_set(node_size.width),
                        height: constants.container_size.height.into(),
                    },
                    vertical_margins_are_collapsible: Line::FALSE,
                },
            );
            let last = child.participates_in_last_baseline_alignment(constants.dir);
            let own = if child.baseline_x_flags & 4 != 0 {
                if last {
                    output.last_or_first_x()
                } else {
                    output.baselines_x.first
                }
            } else {
                None
            };
            let synthesized = if child.baseline_x_flags & 2 != 0 {
                output.size.width / 2.0
            } else {
                0.0
            };
            let mut baseline = own.unwrap_or(synthesized);
            if child.overflow.x.is_scroll_container() {
                baseline = baseline.clamp(0.0, output.size.width);
            }
            let distance = if end {
                output.size.width - baseline + child.margin.right
            } else {
                baseline + child.margin.left
            };
            child.column_baseline = Some((end, distance));
        }
    }
}

pub(super) fn maxima(items: &[FlexItem]) -> [f32; 2] {
    let mut max = [f32::NEG_INFINITY; 2];
    for child in items {
        if let Some((end, distance)) = child.column_baseline {
            max[usize::from(end)] = max[usize::from(end)].max(distance);
        }
    }
    max
}

pub(super) fn cross_size(items: &[FlexItem], constants: &AlgoConstants) -> f32 {
    let max = maxima(items);
    items
        .iter()
        .map(|child| {
            let outer = child.hypothetical_outer_size.cross(constants.dir);
            match child.column_baseline {
                Some((end, distance)) => outer + max[usize::from(end)] - distance,
                None => outer,
            }
        })
        .fold(0.0, f32::max)
}

pub(super) fn offset(
    child: &FlexItem,
    max: [f32; 2],
    line: f32,
    constants: &AlgoConstants,
) -> Option<f32> {
    if !constants.is_column {
        return None;
    }
    let (end, distance) = child.column_baseline?;
    let shim = max[usize::from(end)] - distance;
    Some(if end {
        line - child.outer_target_size.width - shim
    } else {
        shim
    })
}
