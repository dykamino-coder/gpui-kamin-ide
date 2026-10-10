//! Доли размеров атома: разрешение относительно обёртки и блока.

use crate::dom::Element;
use crate::render::{content_sized_wraps, replaced_tag};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Доли коробки по содержимому, решённые от содержащего блока (см. вызов в
/// `blocks`). `None` — решать нечего или блок не блочный: у гибкого и
/// сеточного родителя размеры приходят от раскладки.
pub(crate) fn pct_resolved_for_wrapper(e: &Element, inherited: &Computed) -> Option<Element> {
    if !content_sized_wraps(e) {
        return None;
    }
    pct_resolved_against_block(e, inherited)
}

/// Доли высоты и отступов, решённые от блочного содержащего блока с
/// известными сторонами. Нужны там, где между коробкой и её содержащим
/// блоком стоит наша служебная обёртка (сетка `content_sized`, строка
/// абзаца у `inline-block`), и раскладка решала бы долю от неё.
pub(crate) fn pct_resolved_against_block(e: &Element, inherited: &Computed) -> Option<Element> {
    if replaced_tag(e) || e.style.vertical == Some(true) {
        return None;
    }
    if !matches!(inherited.display, None | Some(Display::Block)) || inherited.vertical == Some(true)
    {
        return None;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Опора высоты — СОДЕРЖИМОЕ родителя: при `box-sizing: border-box`
    // заданная высота включает его отступы и рамку.
    let base_h = match inherited.height {
        Some(Len::Px(h)) if inherited.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                h - px_of(inherited.padding.top)
                    - px_of(inherited.padding.bottom)
                    - px_of(b.top)
                    - px_of(b.bottom),
            )
        }
        Some(Len::Px(h)) => Some(h),
        // Высота из `aspect-ratio` при ширине в точках — определённая
        // (css-sizing-4 §5.1, тот же признак `ratio_height` в
        // `inline::inherit`): `.outer {width: 200px; aspect-ratio: 2/1}` даёт
        // доле детей 100 (`intrinsic-percent-replaced-015/016`). Только
        // `content-box`: соотношение тогда — у содержимого.
        None | Some(Len::Auto) if inherited.border_box != Some(true) => {
            match (inherited.aspect_ratio, inherited.width) {
                (Some(r), Some(Len::Px(w))) if r.is_finite() && r > 0.0 => Some(w / r),
                _ => None,
            }
        }
        _ => None,
    }
    .filter(|h| *h >= 0.0);
    let base_w = match inherited.width {
        Some(Len::Px(w)) if inherited.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                w - px_of(inherited.padding.left)
                    - px_of(inherited.padding.right)
                    - px_of(b.left)
                    - px_of(b.right),
            )
        }
        Some(Len::Px(w)) => Some(w),
        _ => None,
    }
    .filter(|w| *w >= 0.0);
    let of = |l: Option<Len>, base: Option<f32>| match (l, base) {
        (Some(Len::Pct(k)), Some(b)) => Some(Some(Len::Px(k * b))),
        _ => None,
    };
    let mut copy = e.clone();
    let mut changed = false;
    if let Some(v) = of(e.style.height, base_h) {
        copy.style.height = v;
        changed = true;
    }
    for (dst, src) in [
        (&mut copy.style.padding.top, e.style.padding.top),
        (&mut copy.style.padding.bottom, e.style.padding.bottom),
        (&mut copy.style.padding.left, e.style.padding.left),
        (&mut copy.style.padding.right, e.style.padding.right),
    ] {
        if let Some(v) = of(src, base_w) {
            *dst = v;
            changed = true;
        }
    }
    changed.then_some(copy)
}
