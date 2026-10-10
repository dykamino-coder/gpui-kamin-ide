//! Явно заданные (не унаследованные) длины под zoom: что масштабировать у элемента и что запомнить для потомков.

use super::*;

/// Явное `inherit` на ненаследуемых размерах и краях (`explicit-inherit/*`).
/// Родитель здесь — его СОБСТВЕННЫЙ стиль; для ненаследуемых свойств он и
/// есть вычисленный, и уже домножен на зум родителя.
pub(super) fn explicit(c: &mut Computed, parent: Option<&Computed>, own: f32) {
    let Some(p) = parent else { return };
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(Len::Px(v * own)),
        _ => None,
    };
    if c.width_inherit
        && let Some(v) = px(p.width)
    {
        c.width = Some(v);
        c.width_inherit = false;
    }
    if c.height_inherit
        && let Some(v) = px(p.height)
    {
        c.height = Some(v);
        c.height_inherit = false;
    }
    // Порядок разрядов — как в `inline::inherit`: min-w, min-h, max-w, max-h.
    let minmax = [p.min_width, p.min_height, p.max_width, p.max_height];
    for (i, from) in minmax.into_iter().enumerate() {
        if c.minmax_inherit[i]
            && let Some(v) = px(from)
        {
            match i {
                0 => c.min_width = Some(v),
                1 => c.min_height = Some(v),
                2 => c.max_width = Some(v),
                _ => c.max_height = Some(v),
            }
            c.minmax_inherit[i] = false;
        }
    }
    // `padding: inherit` целиком: снимается, только если все стороны родителя
    // в точках или пусты — иначе слияние копирует весь набор, как прежде.
    if c.padding_inherit
        && [
            p.padding.top,
            p.padding.right,
            p.padding.bottom,
            p.padding.left,
        ]
        .iter()
        .all(|l| matches!(l, None | Some(Len::Px(_))))
    {
        c.padding = Sides {
            top: px(p.padding.top),
            right: px(p.padding.right),
            bottom: px(p.padding.bottom),
            left: px(p.padding.left),
        };
        c.padding_inherit = false;
    }
    per_side(&mut c.padding_inherit_side, &mut c.padding, &p.padding, own);
    per_side(&mut c.margin_inherit, &mut c.margin, &p.margin, own);
    per_side(&mut c.inset_inherit, &mut c.inset, &p.inset, own);
    per_side(
        &mut c.border_inherit_w,
        &mut c.border_width,
        &p.border_width,
        own,
    );
    // `outline-width`/`outline-offset: inherit` и `background-size: inherit`
    // живут разрядами `inherit_bits`, и слияние (`inline::inherit`) копирует
    // точки родителя БЕЗ своего множителя (`zoom/outline-width`,
    // `outline-offset`, `background-size`: вторая коробка группы с зумом 2).
    // Значение родителя — его собственный стиль, уже с его зумом, × `own`;
    // разряд снимается. Не в точках — разряд остаётся, как прежде.
    use crate::style::computed::inh;
    if c.inherit_bits & (inh::OUTLINE_W | inh::OUTLINE_O) != 0 {
        let from = p.outline.unwrap_or_default();
        let mut o = c.outline.unwrap_or_default();
        let mut hit = false;
        if c.inherit_bits & inh::OUTLINE_W != 0
            && let Some(v) = px(from.width)
        {
            o.width = Some(v);
            c.inherit_bits &= !inh::OUTLINE_W;
            hit = true;
        }
        if c.inherit_bits & inh::OUTLINE_O != 0
            && let Some(v) = px(from.offset)
        {
            o.offset = Some(v);
            c.inherit_bits &= !inh::OUTLINE_O;
            hit = true;
        }
        if hit {
            c.outline = Some(o);
        }
    }
    if c.inherit_bits & inh::BG_SIZE != 0
        && let crate::style::computed::BgSize::Fixed(w, h) = p.bg_size
    {
        let k = |l: Option<Len>| px(l).or(l);
        c.bg_size = crate::style::computed::BgSize::Fixed(k(w), k(h));
        c.inherit_bits &= !inh::BG_SIZE;
    }
}

/// Запомнить для потомков собственные наследуемые длины в точках. Заданное
/// не в точках СБРАСЫВАЕТ запомненное: потомок унаследует не точки, и
/// домножать нечего (число `line-height`, `em` — считаются от его кегля).
pub(super) fn remember(c: &Computed, k: &mut Carried) {
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let keep = |slot: &mut Option<f32>, l: Option<Len>| {
        if l.is_some() {
            *slot = px(l);
        }
    };
    keep(&mut k.line_height, c.line_height);
    keep(&mut k.letter_spacing, c.letter_spacing);
    keep(&mut k.word_spacing, c.word_spacing);
    keep(&mut k.text_indent, c.text_indent);
    keep(&mut k.tab_size, c.tab_size_len);
    if c.text_shadow.is_some() {
        k.text_shadow = c.text_shadow;
    }
    if let Some((row, col)) = c.border_spacing {
        k.border_spacing = Some((px(row), px(col)));
    }
    match c.underline_offset {
        Some(crate::style::computed::DecorLen::Px(v)) => k.underline_offset = Some(v),
        Some(_) => k.underline_offset = None,
        None => {}
    }
}
