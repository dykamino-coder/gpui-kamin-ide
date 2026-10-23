//! Явно заданные (не унаследованные) длины под zoom: что масштабировать у элемента и что запомнить
//! для потомков.

use super::*;
use crate::style::values::value::px_strict;

/// Явное `inherit` на ненаследуемых размерах и краях (`explicit-inherit/*`).
/// Родитель здесь — его СОБСТВЕННЫЙ стиль; для ненаследуемых свойств он и
/// есть вычисленный, и уже домножен на зум родителя. Значение родителя в
/// точках × `own` становится своим, и свойство уходит из каскадного
/// `inherit` (`inherit_longhands`); не в точках — остаётся, его скопирует
/// стадия вычисленных значений (доля и `auto` зуму безразличны).
pub(super) fn explicit(c: &mut Computed, parent: Option<&Computed>, own: f32) {
    use crate::style::css::{BoxGroup as G, LonghandId as L};
    let Some(p) = parent else { return };
    if c.inherit_longhands.is_empty() {
        return;
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(Len::Px(v * own)),
        _ => None,
    };
    let sizes = [
        p.width,
        p.height,
        p.min_width,
        p.min_height,
        p.max_width,
        p.max_height,
    ];
    for (i, from) in sizes.into_iter().enumerate() {
        let id = L::physical(G::Size, i);
        if c.inherit_longhands.contains(id)
            && let Some(v) = px(from)
        {
            *match i {
                0 => &mut c.width,
                1 => &mut c.height,
                2 => &mut c.min_width,
                3 => &mut c.min_height,
                4 => &mut c.max_width,
                _ => &mut c.max_height,
            } = Some(v);
            c.inherit_longhands.set(id, false);
        }
    }
    for group in [G::Padding, G::Margin, G::Inset, G::BorderWidth] {
        let from = match group {
            G::Padding => p.padding,
            G::Margin => p.margin,
            G::Inset => p.inset,
            _ => p.border_width,
        };
        for i in 0..4 {
            let id = L::physical(group, i);
            if c.inherit_longhands.contains(id)
                && let Some(Len::Px(v)) = side(&from, i)
            {
                let into = match group {
                    G::Padding => &mut c.padding,
                    G::Margin => &mut c.margin,
                    G::Inset => &mut c.inset,
                    _ => &mut c.border_width,
                };
                set_side(into, i, Len::Px(v * own));
                c.inherit_longhands.set(id, false);
            }
        }
    }
    // `outline-width`/`outline-offset: inherit` и `background-size: inherit`:
    // стадия вычисленных значений копирует точки родителя БЕЗ своего
    // множителя (`zoom/outline-width`, `outline-offset`, `background-size`:
    // вторая коробка группы с зумом 2).
    let from = p.outline.clone().unwrap_or_default();
    let mut o = c.outline.clone().unwrap_or_default();
    let mut hit = false;
    if c.inherit_longhands.contains(L::OutlineWidth)
        && let Some(v) = px(from.width)
    {
        o.width = Some(v);
        c.inherit_longhands.set(L::OutlineWidth, false);
        hit = true;
    }
    if c.inherit_longhands.contains(L::OutlineOffset)
        && let Some(v) = px(from.offset)
    {
        o.offset = Some(v);
        c.inherit_longhands.set(L::OutlineOffset, false);
        hit = true;
    }
    if hit {
        c.outline = Some(o);
    }
    if c.inherit_longhands.contains(L::BackgroundSize)
        && let crate::style::computed::BgSize::Fixed(w, h) = p.bg_size
    {
        let k = |l: Option<Len>| px(l).or(l);
        c.bg_size = crate::style::computed::BgSize::Fixed(k(w), k(h));
        c.inherit_longhands.set(L::BackgroundSize, false);
    }
}

/// Запомнить для потомков собственные наследуемые длины в точках. Заданное
/// не в точках СБРАСЫВАЕТ запомненное: потомок унаследует не точки, и
/// домножать нечего (число `line-height`, `em` — считаются от его кегля).
pub(super) fn remember(c: &Computed, k: &mut Carried) {
    let px = px_strict;
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
        k.text_shadow = c.text_shadow.clone();
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
