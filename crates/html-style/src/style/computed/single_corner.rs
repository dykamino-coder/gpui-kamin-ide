//! Допустимость вычисленных значений и геометрия одного круглого CSS-угла.

use crate::style::computed::{BgClip, Computed, Overflow};
use crate::style::values::value::Len;

fn length(l: Option<Len>) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

pub fn uniform(s: crate::style::computed::Sides) -> Option<f32> {
    let a = [s.top, s.right, s.bottom, s.left].map(length);
    let first = a[0]?;
    a.into_iter().all(|v| v == Some(first)).then_some(first)
}

pub fn corner(c: &Computed, size: (f32, f32)) -> Option<(usize, f32)> {
    let mut found = None;
    for (i, l) in [
        c.reset_border.radius.tl,
        c.reset_border.radius.tr,
        c.reset_border.radius.br,
        c.reset_border.radius.bl,
    ]
    .into_iter()
    .enumerate()
    {
        let r = match l {
            Some(Len::Pct(p)) if p > 0.0 && (size.0 - size.1).abs() < 0.001 => p * size.0,
            Some(Len::Px(v)) => v,
            None | Some(Len::Pct(0.0)) => 0.0,
            _ => return None,
        };
        if r > 0.0 {
            if found.is_some() {
                return None;
            }
            found = Some((i, r.min(size.0).min(size.1)));
        }
    }
    found
}

#[inline(never)]
pub fn eligible(c: &Computed) -> bool {
    eligible_background(c, crate::style::background_paint::BackgroundPaint::Css)
}

/// Выбор совместного примитива по собственному used-фону, без изменения CSS.
pub fn eligible_background(
    c: &Computed,
    background: crate::style::background_paint::BackgroundPaint,
) -> bool {
    let special = background.paints()
        && (c.reset_background.gradient.is_some()
            || c.reset_background.bg_image.is_some()
            || !c.reset_background.bg_lists.is_empty()
            || matches!(
                c.reset_background.bg_clip,
                Some(BgClip::Text | BgClip::BorderArea)
            ));
    geometry_eligible(
        c,
        special,
        !c.reset_effects.shadows.is_empty(),
        c.used_uniform_border_color().is_some(),
    )
}

pub fn box_eligible(c: &super::BoxStyle) -> bool {
    let bg = c.paint_background();
    let special = c.background_paint.paints()
        && (bg.gradient.is_some()
            || bg.top.image.is_some()
            || bg.layers.is_some()
            || matches!(bg.top.clip, Some(BgClip::Text | BgClip::BorderArea)));
    geometry_eligible(
        c,
        special,
        !c.paint_shadows().is_empty(),
        c.used_uniform_border_color().is_some(),
    )
}

fn geometry_eligible(c: &Computed, special: bool, shadows: bool, uniform_color: bool) -> bool {
    if special
        || c.reset_border.border_image.is_some()
        || c.reset_border.border_shape.is_some()
        || c.reset_border.corner_shape.is_some()
        || c.reset_border.radius_ell.is_some()
        || shadows
        || !c.reset_effects.inset_shadows.is_empty()
        || c.reset_context.contain_paint == Some(true)
        || [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y]
            .into_iter()
            .any(|v| !matches!(v, None | Some(Overflow::Visible)))
    {
        return false;
    }
    let Some(b) = uniform(c.borders()) else {
        return false;
    };
    let Some(p) = uniform(c.reset_box_layout.padding) else {
        return false;
    };
    if b > 0.0
        && c.reset_border
            .border_side_styles
            .iter()
            .any(|v| *v != Some(9))
    {
        return false;
    }
    if !uniform_color {
        return false;
    }
    let (Some(Len::Px(w)), Some(Len::Px(h))) =
        (c.reset_box_layout.width, c.reset_box_layout.height)
    else {
        return false;
    };
    let add = if c.reset_border.border_box == Some(true) {
        0.0
    } else {
        2.0 * (p + b)
    };
    let size = (w + add, h + add);
    corner(c, size).is_some_and(|(_, r)| r > size.0.min(size.1) * 0.5)
}
