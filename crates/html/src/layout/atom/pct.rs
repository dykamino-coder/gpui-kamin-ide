//! Доли размеров атома: разрешение относительно обёртки и блока.

use crate::dom::Element;
use crate::layout::box_kind::replaced_tag;
use crate::layout::intrinsic_wrap::content_sized_wraps;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};

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
    if replaced_tag(e) || e.style.cascaded().wm().vertical() {
        return None;
    }
    if !matches!(
        inherited.reset_box_layout.display,
        None | Some(Display::Block)
    ) || inherited.wm().vertical()
    {
        return None;
    }
    let px_of = px_lossy_zero;
    // Опора высоты — СОДЕРЖИМОЕ родителя: при `box-sizing: border-box`
    // заданная высота включает его отступы и рамку.
    let base_h = match inherited.reset_box_layout.height {
        Some(Len::Px(h)) if inherited.reset_border.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                h - px_of(inherited.reset_box_layout.padding.top)
                    - px_of(inherited.reset_box_layout.padding.bottom)
                    - px_of(b.top)
                    - px_of(b.bottom),
            )
        }
        Some(Len::Px(h)) => Some(h),
        // Высота из `aspect-ratio` при ширине в точках — определённая
        // (css-sizing-4 §5.1, тот же признак `ratio_height` в
        // `style::cascade::inherit`): `.outer {width: 200px; aspect-ratio: 2/1}` даёт
        // доле детей 100 (`intrinsic-percent-replaced-015/016`). Только
        // `content-box`: соотношение тогда — у содержимого.
        None | Some(Len::Auto) if inherited.reset_border.border_box != Some(true) => {
            match (
                inherited.reset_shapes.aspect_ratio,
                inherited.reset_box_layout.width,
            ) {
                (Some(r), Some(Len::Px(w))) if r.is_finite() && r > 0.0 => Some(w / r),
                _ => None,
            }
        }
        _ => None,
    }
    .filter(|h| *h >= 0.0);
    let base_w = match inherited.reset_box_layout.width {
        Some(Len::Px(w)) if inherited.reset_border.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                w - px_of(inherited.reset_box_layout.padding.left)
                    - px_of(inherited.reset_box_layout.padding.right)
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
    // Решить доли по исходному стилю и копировать элемент только при правке:
    // обход атомов зовёт это для каждого `inline-block`, и копия на каждом
    // (вместе с общей группой размеров) стоила сборке столько же, сколько остальное.
    let used = e.style.used();
    let own = &used.reset_box_layout;
    let height = of(own.height, base_h);
    let padding = [
        own.padding.top,
        own.padding.bottom,
        own.padding.left,
        own.padding.right,
    ]
    .map(|side| of(side, base_w));
    drop(used);
    if height.is_none() && padding.iter().all(Option::is_none) {
        return None;
    }
    let mut copy = e.clone();
    let layout = &mut *copy.style.used_mut().reset_box_layout;
    if let Some(v) = height {
        layout.height = v;
    }
    let sides = [
        &mut layout.padding.top,
        &mut layout.padding.bottom,
        &mut layout.padding.left,
        &mut layout.padding.right,
    ];
    for (dst, v) in sides.into_iter().zip(padding) {
        if let Some(v) = v {
            *dst = v;
        }
    }
    Some(copy)
}

#[cfg(test)]
mod tests {
    use crate::paint::contract_tests::rendered::quads;

    fn size_of(html: &str, colour: u32) -> (f32, f32) {
        quads(html, 300., 200.)
            .into_iter()
            .find(|(_, c)| c.to_rgb() == gpui::rgb(colour))
            .map(|(b, _)| (f32::from(b.size.width), f32::from(b.size.height)))
            .expect("painted atom")
    }

    #[test]
    fn percentages_of_an_atom_resolve_against_its_block() {
        let html = "<style>body{margin:0}</style><div style='width:200px;height:100px'>\
                    <span style='display:inline-block;width:10px;height:50%;\
                    padding-left:10%;background:red'></span></div>";
        assert_eq!(size_of(html, 0xff0000), (30.0, 50.0));
    }

    #[test]
    fn an_atom_without_percentages_keeps_its_own_sizes() {
        let html = "<style>body{margin:0}</style><div style='width:200px;height:100px'>\
                    <span style='display:inline-block;width:10px;height:20px;\
                    padding-left:4px;background:blue'></span></div>";
        assert_eq!(size_of(html, 0x0000ff), (14.0, 20.0));
    }
}
