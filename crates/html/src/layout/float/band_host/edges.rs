//! Поля потомков измеряемого float-хоста: px, процент от ширины containing block
//! или `em` от размера шрифта бокса.

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
    // The box's font size matters only to an `em` margin.
    let em = band_em(c, em);
    let edge = |l: &Option<Len>| match l {
        Some(Len::Em(_)) => band_edge(l, em?),
        _ => band_edge(l, 0.0),
    };
    Some([
        edge(&c.margin.top)?,
        edge(&c.margin.right)?,
        edge(&c.margin.bottom)?,
        edge(&c.margin.left)?,
    ])
}
