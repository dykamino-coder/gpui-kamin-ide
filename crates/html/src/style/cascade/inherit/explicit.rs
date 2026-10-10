//! inherit_stage, этап явного `inherit` у ненаследуемых свойств: биты inherit_bits, поля, отступы, размеры, inset, фон и background_rcs.

use super::*;

pub(super) fn inherit_explicit(parent: &Computed, own: &Computed, c: &mut Computed) {
    // Ненаследуемые свойства со словом `inherit`: значение родителя берётся
    // целиком (§6.2.1). Разбор их слотов слово не выражает — там оно давало
    // умолчание или роняло объявление.
    if own.inherit_bits != 0 {
        use crate::style::computed::inh;
        let on = |b: u32| own.inherit_bits & b != 0;
        if on(inh::BG_REPEAT) {
            c.bg_repeat = parent.bg_repeat;
        }
        if on(inh::Z_INDEX) {
            c.z_index = parent.z_index;
        }
        if own.inherit_bits & (inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S | inh::OUTLINE_O)
            != 0
        {
            let from = parent.outline.unwrap_or_default();
            let mut o = c.outline.unwrap_or_default();
            if on(inh::OUTLINE_W) {
                o.width = from.width;
            }
            if on(inh::OUTLINE_C) {
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving):
                // у родителя он хранится ПУСТЫМ слотом, и наследовать надо
                // пустоту — цвет возьмётся от СВОЕГО текста, а не от чужого
                // (`outline-019`: родитель красный, ребёнок зелёный).
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
        }
        if on(inh::BG_IMAGE) {
            c.bg_image = parent.bg_image.clone();
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
        if on(inh::BG_POS) {
            c.bg_pos = parent.bg_pos;
        }
        if on(inh::CLIP) {
            c.clip_rect = parent.clip_rect;
        }
        if on(inh::BG_ORIGIN) {
            c.bg_origin = parent.bg_origin;
        }
        if on(inh::BG_CLIP) {
            c.bg_clip = parent.bg_clip;
        }
        if on(inh::BG_SIZE) {
            c.bg_size = parent.bg_size;
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
    if own.background_inherit {
        c.background = parent.background;
        c.background_rcs = own.background_rcs.clone().or(parent.background_rcs.clone());
        if own.background_all_inherit {
            c.bg_image = parent.bg_image.clone();
            c.bg_repeat = parent.bg_repeat;
            c.bg_pos = parent.bg_pos;
            c.bg_size = parent.bg_size;
            c.bg_fixed = parent.bg_fixed;
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
    }
    // Относительный цвет решается ЗДЕСЬ: только теперь известен цвет самого
    // элемента. Функция остаётся в поле — её унаследуют дети и решат своим
    // цветом заново.
    if let Some(expr) = c.background_rcs.clone() {
        let current = c.color.unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        if let Some(resolved) = crate::style::values::color_space::resolve_relative(&expr, current)
        {
            c.background = Some(resolved);
        }
    }
}
