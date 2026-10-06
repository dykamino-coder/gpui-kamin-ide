//! Main-axis fit sizes clamp independent intrinsic probes before flexing.
use super::{AlgoConstants, FlexItem};
use crate::compute::common::sizing_keyword::{resolve_sizing_keyword, SizingKeywordResolution};
use crate::geometry::{Line, Size};
use crate::style::{AvailableSpace, Dimension};
use crate::style_helpers::TaffyMaxContent;
use crate::tree::{LayoutFlexboxContainer, LayoutPartialTreeExt, SizingMode};
use crate::{CompactLength};
use crate::util::MaybeMath;

pub(super) enum Basis {
    Size(f32, bool),
    Space(Option<AvailableSpace>),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    tree: &mut impl LayoutFlexboxContainer,
    child: &FlexItem,
    constants: &AlgoConstants,
    basis_style: Dimension,
    numeric_basis: Option<f32>,
    known: Size<Option<f32>>,
    parent: Size<Option<f32>>,
    cross_space: AvailableSpace,
    box_adjustment: f32,
) -> Basis {
    if basis_style.is_content() {
        return Basis::Space(None);
    }
    let style = if basis_style.is_sizing_keyword() {
        basis_style
    } else {
        if let Some(size) = numeric_basis.or(child.size.main(constants.dir)) {
            return Basis::Size(size, true);
        }
        child.size_style.main(constants.dir)
    };
    let basis = constants.pct_basis().main(constants.dir);
    let stretch = basis.maybe_sub(child.margin.main_axis_sum(constants.dir));
    let available = match resolve_sizing_keyword(style, stretch, basis) {
        Some(SizingKeywordResolution::Exact(size)) => return Basis::Size(size, true),
        Some(SizingKeywordResolution::Measure(space)) => space,
        None => return Basis::Space(None),
    };
    let fit = matches!(style.tag(), CompactLength::FIT_CONTENT_PX_TAG
        | CompactLength::FIT_CONTENT_PERCENT_TAG | CompactLength::FIT_CONTENT_KEYWORD_TAG);
    let Some(mut argument) = available.into_option().filter(|_| fit) else {
        return Basis::Space(Some(available));
    };
    if style.tag() != CompactLength::FIT_CONTENT_KEYWORD_TAG {
        argument += box_adjustment;
    }
    let mut probe = |main| tree.measure_child_size(
        child.node,
        known.with_main(constants.dir, None),
        parent,
        Size::MAX_CONTENT.with_main(constants.dir, main).with_cross(constants.dir, cross_space),
        SizingMode::ContentSize,
        constants.dir.main_axis(),
        Line::FALSE,
    );
    let minimum = probe(AvailableSpace::MinContent);
    let maximum = probe(AvailableSpace::MaxContent);
    // A fixed argument still depends on content, so this basis is not definite.
    Basis::Size(minimum.max(argument.min(maximum)), false)
}

#[cfg(test)]
#[path = "../tree/flex_main_fit_tests.rs"]
mod tests;
