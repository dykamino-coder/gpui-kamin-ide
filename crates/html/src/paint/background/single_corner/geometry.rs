//! Select and inset a single circular CSS corner in its original border box.

use crate::style::computed::{BgClip, Computed, Overflow};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, px};

fn length(l: Option<Len>) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

pub(super) fn uniform(s: crate::style::computed::Sides) -> Option<f32> {
    let a = [s.top, s.right, s.bottom, s.left].map(length);
    let first = a[0]?;
    a.into_iter().all(|v| v == Some(first)).then_some(first)
}

pub(super) fn corner(c: &Computed, size: (f32, f32)) -> Option<(usize, f32)> {
    let mut found = None;
    for (i, l) in [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl]
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
pub(crate) fn eligible(c: &Computed) -> bool {
    if c.gradient.is_some()
        || c.bg_image.is_some()
        || c.border_image.is_some()
        || c.border_shape.is_some()
        || c.corner_shape.is_some()
        || c.radius_ell.is_some()
        || !c.bg_lists.is_empty()
        || !c.shadows.is_empty()
        || !c.inset_shadows.is_empty()
        || c.contain_paint == Some(true)
        || matches!(c.bg_clip, Some(BgClip::Text | BgClip::BorderArea))
        || [c.overflow_x, c.overflow_y]
            .into_iter()
            .any(|v| !matches!(v, None | Some(Overflow::Visible)))
    {
        return false;
    }
    let Some(b) = uniform(c.borders()) else {
        return false;
    };
    let Some(p) = uniform(c.padding) else {
        return false;
    };
    if b > 0.0 && c.border_side_styles.iter().any(|v| *v != Some(9)) {
        return false;
    }
    let colors: Vec<_> = c.border_colors.iter().flatten().collect();
    if colors.iter().any(|v| *v != colors[0]) {
        return false;
    }
    let (Some(Len::Px(w)), Some(Len::Px(h))) = (c.width, c.height) else {
        return false;
    };
    let add = if c.border_box == Some(true) {
        0.0
    } else {
        2.0 * (p + b)
    };
    let size = (w + add, h + add);
    corner(c, size).is_some_and(|(_, r)| r > size.0.min(size.1) * 0.5)
}

pub(super) fn inset(r: Bounds<Pixels>, n: f32) -> Bounds<Pixels> {
    Bounds::from_corners(
        gpui::point(r.left() + px(n), r.top() + px(n)),
        gpui::point(r.right() - px(n), r.bottom() - px(n)),
    )
}

pub(super) fn virtual_circle(r: Bounds<Pixels>, index: usize, radius: f32) -> Bounds<Pixels> {
    let x = if matches!(index, 1 | 2) {
        r.right() - px(2.0 * radius)
    } else {
        r.left()
    };
    let y = if matches!(index, 2 | 3) {
        r.bottom() - px(2.0 * radius)
    } else {
        r.top()
    };
    Bounds {
        origin: gpui::point(x, y),
        size: gpui::size(px(2.0 * radius), px(2.0 * radius)),
    }
}

pub(super) fn square(r: Bounds<Pixels>, i: usize, radius: f32) -> Bounds<Pixels> {
    let x = if matches!(i, 1 | 2) {
        r.right() - px(radius)
    } else {
        r.left()
    };
    let y = if matches!(i, 2 | 3) {
        r.bottom() - px(radius)
    } else {
        r.top()
    };
    Bounds {
        origin: gpui::point(x, y),
        size: gpui::size(px(radius), px(radius)),
    }
}
