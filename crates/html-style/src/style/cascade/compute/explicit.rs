//! Стадия вычисленных значений, каскадное `inherit` ненаследуемых свойств вне реестра длинных
//! свойств (разряды `inherit_bits`); свойства реестра — `longhands`.

use super::*;

pub(super) fn inherit_explicit(parent: &Computed, own: &Computed, c: &mut Computed) {
    if own.reset_text_box.text_overflow_inherit {
        c.reset_text_box.ellipsis = parent.reset_text_box.ellipsis;
        c.reset_text_box.overflow_marker = parent.reset_text_box.overflow_marker.clone();
    }
    // Ненаследуемые свойства со словом `inherit`: значение родителя берётся
    // целиком (§6.2.1). Разбор их слотов слово не выражает — там оно давало
    // умолчание или роняло объявление.
    if own.reset_cascade.inherit_bits != 0 {
        use crate::style::computed::inh;
        let on = |b: u32| own.reset_cascade.inherit_bits & b != 0;
        if on(inh::Z_INDEX) {
            c.reset_positioned.z_index = parent.reset_positioned.z_index;
        }
        if on(inh::CORNER_SHAPE) {
            c.reset_border.corner_shape = parent.reset_border.corner_shape;
        }
        if on(inh::RADIUS) {
            c.reset_border.radius = parent.reset_border.radius;
            c.reset_border.radius_ell = parent.reset_border.radius_ell;
        }
        if on(inh::DISPLAY) {
            c.reset_box_layout.display = parent.reset_box_layout.display;
            c.reset_context.inline_display = parent.reset_context.inline_display;
            c.reset_svg_math.math_layout = parent.reset_svg_math.math_layout;
        }
        if on(inh::CLIP) {
            c.reset_mask.clip_rect = parent.reset_mask.clip_rect;
            c.reset_mask.clip_len = parent.reset_mask.clip_len;
        }
        if on(inh::TRANSFORM) {
            c.reset_transforms.transform = parent.reset_transforms.transform;
        }
        if on(inh::TRANSFORM_ORIGIN) {
            c.reset_transforms.transform_origin = parent.reset_transforms.transform_origin;
            c.reset_transforms.transform_origin_px = parent.reset_transforms.transform_origin_px;
            c.reset_transforms.transform_origin_z = parent.reset_transforms.transform_origin_z;
        }
        if on(inh::CLIP_MARGIN) {
            c.reset_mask.clip_margin = parent.reset_mask.clip_margin;
            c.reset_mask.clip_margin_box = parent.reset_mask.clip_margin_box;
        }
    }
}
