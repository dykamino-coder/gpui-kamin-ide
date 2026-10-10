//! Resolve authored leaf sizes before ratio transfer without overriding definite axes.
use crate::{CoreStyle, MaybeMath, MaybeResolve, Size};

pub(super) fn resolve(
    style: &impl CoreStyle,
    parent: Size<Option<f32>>,
    known: Size<Option<f32>>,
    box_adjustment: Size<f32>,
    calc: &impl Fn(*const (), f32) -> f32,
) -> (
    Size<Option<f32>>,
    Size<Option<f32>>,
    Size<Option<f32>>,
    Option<f32>,
) {
    let ratio = style.aspect_ratio();
    let authored = style.size().maybe_resolve(parent, calc);
    let (minimum, maximum) = crate::compute::common::aspect_ratio_constraints::transfer(
        known.maybe_sub(box_adjustment).or(authored),
        style.min_size().maybe_resolve(parent, calc),
        style.max_size().maybe_resolve(parent, calc),
        ratio,
    );
    (
        known.or(authored
            .maybe_apply_aspect_ratio(ratio)
            .maybe_add(box_adjustment)),
        minimum.maybe_add(box_adjustment),
        maximum.maybe_add(box_adjustment),
        ratio,
    )
}
