//! Cross keywords constrain both flex-basis and hypothetical-cross probes.
use super::{item_known_dimension_definiteness, AlgoConstants, FlexItem};
use crate::compute::common::sizing_keyword::{resolve_sizing_keyword, SizingKeywordResolution};
use crate::geometry::{Line, Size};
use crate::style::{AvailableSpace, CoreStyle};
use crate::style_helpers::TaffyMaxContent;
use crate::tree::{LayoutFlexboxContainer, LayoutInput, LayoutPartialTreeExt, RunMode, SizingMode};
use crate::util::MaybeMath;
use crate::{BoxSizing, CompactLength};

pub(super) fn available(
    tree: &mut impl LayoutFlexboxContainer,
    child: &FlexItem,
    constants: &AlgoConstants,
    known_main: Option<f32>,
    fallback: AvailableSpace,
) -> AvailableSpace {
    let dir = constants.dir;
    let stretch = constants
        .node_inner_size
        .cross(dir)
        .map(|size| constants.divided_cross_space(size))
        .maybe_sub(child.margin.cross_axis_sum(dir))
        .maybe_max(0.0);
    match resolve_sizing_keyword(
        child.size_style.cross(dir),
        stretch,
        constants.pct_basis().cross(dir),
        |val, basis| tree.calc(val, basis),
    ) {
        Some(SizingKeywordResolution::Measure(space)) => {
            let style = child.size_style.cross(dir);
            let fit = matches!(
                style.tag(),
                CompactLength::FIT_CONTENT_PX_TAG
                    | CompactLength::FIT_CONTENT_PERCENT_TAG
                    | CompactLength::FIT_CONTENT_KEYWORD_TAG
            );
            let Some(mut argument) = space.into_option().filter(|_| fit) else {
                return space;
            };
            // Probes return border-box sizes. A functional content-box argument
            // needs its own edges once; the stretch-derived keyword cap already includes them.
            if style.tag() != CompactLength::FIT_CONTENT_KEYWORD_TAG
                && tree.get_core_container_style(child.node).box_sizing() == BoxSizing::ContentBox
            {
                argument += (child.padding + child.border).cross_axis_sum(dir);
            }
            let minimum = probe(
                tree,
                child,
                constants,
                known_main,
                AvailableSpace::MinContent,
            );
            let maximum = probe(
                tree,
                child,
                constants,
                known_main,
                AvailableSpace::MaxContent,
            );
            AvailableSpace::Definite(minimum.max(argument.min(maximum)))
        }
        // Explicit stretch is sized against the flex line in the used-cross pass.
        _ => fallback,
    }
}

pub(super) fn allows_alignment_stretch(child: &FlexItem, constants: &AlgoConstants) -> bool {
    let style = child.size_style.cross(constants.dir);
    // An unresolved percentage retains its automatic-size behavior; intrinsic
    // measurement keywords must never turn available space into a known size.
    style.is_auto()
        || style.is_stretch()
        || (!style.is_sizing_keyword() && style.into_raw().uses_percentage())
}

fn probe(
    tree: &mut impl LayoutFlexboxContainer,
    child: &FlexItem,
    constants: &AlgoConstants,
    known_main: Option<f32>,
    cross_space: AvailableSpace,
) -> f32 {
    tree.compute_child_layout(
        child.node,
        LayoutInput {
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::ContentSize,
            axis: constants.dir.cross_axis().into(),
            known_dimensions: Size::NONE.with_main(constants.dir, known_main),
            known_dimensions_are_definite: item_known_dimension_definiteness(constants, child),
            parent_size: constants.pct_basis(),
            available_space: Size::MAX_CONTENT.with_cross(constants.dir, cross_space),
            vertical_margins_are_collapsible: Line::FALSE,
        },
    )
    .size
    .cross(constants.dir)
}
