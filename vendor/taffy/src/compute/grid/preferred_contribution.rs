//! Bound a preferred grid contribution before transferring it through its ratio.
//! A capped percentage width must not leave an auto row sized for the uncapped width.
use super::types::GridItem;
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt};
use crate::{MaybeMath, MaybeResolve, Size};

pub(super) fn resolve(
    item: &GridItem,
    tree: &impl LayoutPartialTree,
    basis: Size<Option<f32>>,
    box_adjustment: Size<f32>,
) -> Size<Option<f32>> {
    let preferred = item
        .size
        .maybe_resolve(basis, |value, basis| tree.calc(value, basis));
    let minimum = item
        .min_size
        .maybe_resolve(basis, |value, basis| tree.calc(value, basis));
    let maximum = item
        .max_size
        .maybe_resolve(basis, |value, basis| tree.calc(value, basis));
    let transferred = preferred
        .maybe_clamp(minimum, maximum)
        .maybe_apply_aspect_ratio(item.aspect_ratio);
    // Preserve explicitly resolved preferred axes; only an automatic destination
    // receives the ratio derived from its bounded source axis.
    Size {
        width: preferred.width.or(transferred.width),
        height: preferred.height.or(transferred.height),
    }
    .maybe_add(box_adjustment)
}
