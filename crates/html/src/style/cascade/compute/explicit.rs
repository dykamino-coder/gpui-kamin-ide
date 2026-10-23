//! inherit_stage, этап явного `inherit` у ненаследуемых свойств: биты inherit_bits, поля, отступы,
//! размеры, inset (фон — `background`).

use super::*;
use crate::document::state::ValueCx;

pub(super) fn inherit_explicit(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    _vcx: ValueCx<'_>,
) {
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
        if own.inherit_bits & (inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S | inh::OUTLINE_O)
            != 0
        {
            let from = parent.outline.clone().unwrap_or_default();
            let mut o = c.outline.clone().unwrap_or_default();
            if on(inh::OUTLINE_W) {
                o.width = from.width;
            }
            if on(inh::OUTLINE_C) {
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving)
                // и наследуется словом: цвет возьмётся от СВОЕГО текста, а не
                // от чужого (`outline-019`: родитель красный, ребёнок зелёный).
                o.color = from.color;
            }
            if on(inh::OUTLINE_S) {
                o.style = from.style;
            }
            if on(inh::OUTLINE_O) {
                o.offset = from.offset;
                // Метка `inset` — часть значения сдвига и наследуется с ним.
                o.inset = from.inset;
            }
            c.outline = Some(o);
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
    for (i, on) in own.margin_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.margin.top = parent.margin.top,
            1 => c.margin.right = parent.margin.right,
            2 => c.margin.bottom = parent.margin.bottom,
            _ => c.margin.left = parent.margin.left,
        }
    }
    for (i, on) in own.padding_inherit_side.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.padding.top = parent.padding.top,
            1 => c.padding.right = parent.padding.right,
            2 => c.padding.bottom = parent.padding.bottom,
            _ => c.padding.left = parent.padding.left,
        }
    }
    for (i, inherited) in [
        own.width_inherit,
        own.height_inherit,
        own.minmax_inherit[0],
        own.minmax_inherit[1],
        own.minmax_inherit[2],
        own.minmax_inherit[3],
    ]
    .into_iter()
    .enumerate()
    {
        if inherited {
            c.calc_size_basis[i] = parent.calc_size_basis[i];
            if i < 4 {
                c.calc_size[i] = parent.calc_size[i];
            } else {
                c.calc_size_max[i - 4] = parent.calc_size_max[i - 4];
            }
        }
    }
    if own.width_inherit {
        c.width = parent.width;
    }
    if own.height_inherit {
        c.height = parent.height;
    }
    for (i, on) in own.minmax_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.min_width = parent.min_width,
            1 => c.min_height = parent.min_height,
            2 => c.max_width = parent.max_width,
            _ => c.max_height = parent.max_height,
        }
    }
    for (i, on) in own.inset_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.inset.top = parent.inset.top,
            1 => c.inset.right = parent.inset.right,
            2 => c.inset.bottom = parent.inset.bottom,
            _ => c.inset.left = parent.inset.left,
        }
    }
}
