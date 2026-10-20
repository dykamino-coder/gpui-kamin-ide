//! Margins of the measured float host's children: px, a percentage of the
//! containing block width, or `em` of the box's font size.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Поле для измеряемого хоста: точки, доля ширины содержащего блока или
/// `em` по кеглю `em`; `auto` — ноль (у флоата так велит §10.3.5, у куска
/// хвоста — как `px_margin` статического хоста). Прочее (`calc`, `vw`…) —
/// `None`, хост отменяется.
pub(crate) fn band_edge(l: &Option<Len>, em: f32) -> Option<crate::layout::float::band_flow::Edge> {
    use crate::layout::float::band_flow::Edge;
    match l {
        None | Some(Len::Auto) => Some(Edge::Px(0.0)),
        Some(Len::Px(v)) => Some(Edge::Px(*v)),
        Some(Len::Pct(k)) => Some(Edge::Pct(*k)),
        Some(Len::Em(k)) => Some(Edge::Px(*k * em)),
        _ => None,
    }
}

/// Кегль коробки для `em` её полей; относительный `font-size` берёт
/// кегль родителя, в том числе проценты (CSS 2.1 §15.7).
pub(crate) fn band_em(c: &Computed, em: f32) -> Option<f32> {
    match c.font_size {
        None => Some(em),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(k)) | Some(Len::Pct(k)) => Some(k * em),
        _ => None,
    }
}

/// Все четыре поля коробки разрешимы для измеряемого хоста.
pub(crate) fn band_margins(
    c: &Computed,
    em: f32,
) -> Option<[crate::layout::float::band_flow::Edge; 4]> {
    let em = band_em(c, em)?;
    Some([
        band_edge(&c.margin.top, em)?,
        band_edge(&c.margin.right, em)?,
        band_edge(&c.margin.bottom, em)?,
        band_edge(&c.margin.left, em)?,
    ])
}
