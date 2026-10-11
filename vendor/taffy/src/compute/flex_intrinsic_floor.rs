//! Wrapped intrinsic maximum retains the individual minimum despite negative margins.
use super::{sum_axis_gaps, AlgoConstants, FlexItem, FlexLine};
use crate::util::MaybeMath;
fn contribution(child: &FlexItem, constants: &AlgoConstants) -> f32 {
    let padding_border = (child.padding + child.border).main_axis_sum(constants.dir);
    let size = if child.flex_grow == 0.0 && child.flex_shrink == 0.0 {
        child.hypothetical_inner_size.main(constants.dir)
    } else {
        child
            .flex_basis
            .maybe_max(child.min_size.main(constants.dir))
    };
    (size + child.margin.main_axis_sum(constants.dir)).max(padding_border)
}
pub(super) fn line_minimum(lines: &[FlexLine<'_>], constants: &AlgoConstants) -> f32 {
    // Preserve the previous min-content line sum and gaps exactly.
    lines
        .iter()
        .map(|line| {
            line.items
                .iter()
                .map(|child| contribution(child, constants))
                .sum::<f32>()
                + sum_axis_gaps(constants.gap.main(constants.dir), line.items.len())
        })
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0)
}
pub(super) fn item_minimum(lines: &[FlexLine<'_>], constants: &AlgoConstants) -> f32 {
    lines
        .iter()
        .flat_map(|line| line.items.iter())
        .map(|child| contribution(child, constants))
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
#[path = "../tree/flex_negative_margin_intrinsic_tests.rs"]
mod tests;
