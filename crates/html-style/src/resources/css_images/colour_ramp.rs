//! Общий ограниченный кэш вычисленных цветовых переходов всех геометрий.

use super::raster_keys::{Interpolation, stops};
use crate::cache::{BoundedCache, Hinted, MIB, hint};
use crate::style::{
    computed::GradSpace,
    values::{
        color_space::{DirectGradient, GradientRamp},
        value::Color,
    },
};
use std::sync::Arc;

type Key = Hinted<(
    Vec<([crate::cache::FloatKey; 4], crate::cache::FloatKey)>,
    Interpolation,
    bool,
)>;
static RAMPS: BoundedCache<Key, Arc<GradientRamp>> =
    BoundedCache::new(2 * MIB, |r: &Arc<GradientRamp>| r.bytes()).with_key_weight(|k| {
        k.key.1.bytes()
            + k.key.0.len()
                * std::mem::size_of::<([crate::cache::FloatKey; 4], crate::cache::FloatKey)>()
    });

pub(super) fn ramp(
    values: &[(Color, f32)],
    space: GradSpace,
    hue: u8,
    palette: Option<&DirectGradient>,
    use_hsl: bool,
) -> Arc<GradientRamp> {
    let key = Hinted {
        hint: hint((values.len(), space as u8, hue, use_hsl)),
        key: (
            stops(values),
            Interpolation::new(space, hue, palette),
            use_hsl,
        ),
    };
    RAMPS.get_or_insert_with(key, || {
        Arc::new(GradientRamp::new(values, space, hue, palette, use_hsl))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_equivalent_computed_ramps_share_identity() {
        for values in [vec![], vec![(Color::BLACK, 0.0), (Color::BLACK, 1.0)]] {
            let a = ramp(&values, GradSpace::Srgb, 0, None, false);
            let b = ramp(&values.clone(), GradSpace::Srgb, 0, None, false);
            assert!(Arc::ptr_eq(&a, &b));
        }
    }
}
