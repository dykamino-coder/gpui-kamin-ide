//! Стадия вычисленных значений, каскадное `inherit` ненаследуемых свойств вне реестра длинных
//! свойств (разряды `inherit_bits`); свойства реестра — `longhands`.

use super::*;

pub(super) fn inherit_explicit(parent: &Computed, own: &Computed, c: &mut Computed) {
    // Ненаследуемые свойства со словом `inherit`: значение родителя берётся
    // целиком (§6.2.1). Разбор их слотов слово не выражает — там оно давало
    // умолчание или роняло объявление.
    if own.inherit_bits != 0 {
        use crate::style::computed::inh;
        let on = |b: u32| own.inherit_bits & b != 0;
        if on(inh::Z_INDEX) {
            c.z_index = parent.z_index;
        }
        if on(inh::CORNER_SHAPE) {
            c.corner_shape = parent.corner_shape;
        }
        if on(inh::RADIUS) {
            c.radius = parent.radius;
            c.radius_ell = parent.radius_ell;
        }
        if on(inh::DISPLAY) {
            c.display = parent.display;
            c.inline_display = parent.inline_display;
            c.math_layout = parent.math_layout;
        }
        if on(inh::CLIP) {
            c.clip_rect = parent.clip_rect;
        }
        if on(inh::TRANSFORM) {
            c.transform = parent.transform;
        }
        if on(inh::TRANSFORM_ORIGIN) {
            c.transform_origin = parent.transform_origin;
            c.transform_origin_px = parent.transform_origin_px;
            c.transform_origin_z = parent.transform_origin_z;
        }
        if on(inh::CLIP_MARGIN) {
            c.clip_margin = parent.clip_margin;
            c.clip_margin_box = parent.clip_margin_box;
        }
    }
}
