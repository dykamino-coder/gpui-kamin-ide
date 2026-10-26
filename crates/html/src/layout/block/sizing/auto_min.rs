//! Автоминимум отношения сторон: свойства и размеры — отдельные входы раскладки.

use super::BoxDimensions;
use crate::style::computed::{Computed, Overflow, Position};
use crate::style::values::value::Len;

/// Идёт ли `aspect-ratio` автоминимумом (css-sizing-4 §5.2): незамещаемая
/// коробка без прокрутки, ровно одна ось задана в точках, а минимум
/// ratio-зависимой оси не задан явно.
fn set_len(l: Option<Len>) -> bool {
    matches!(l, Some(x) if x != Len::Auto)
}

pub(crate) fn ratio_as_auto_min(c: &Computed, sizes: &BoxDimensions) -> bool {
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    let px_w = matches!(sizes.preferred[0], Some(Len::Px(_)));
    let px_h = matches!(sizes.preferred[1], Some(Len::Px(_)));
    c.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0)
        // `calc-size()` считает раскладка от размера, выведенного
        // соотношением (`vendor/taffy` — `calc_size_derived`, автоминимум):
        // эмуляция явным минимумом спрятала бы соотношение от неё
        // (`calc-size-aspect-ratio-001…004`).
        && c.calc_size.iter().chain(c.calc_size_max.iter()).all(Option::is_none)
        && visible(c.overflow_x)
        && visible(c.overflow_y)
        && !c.scroller
        && !c.flex_item_ratio
        // Абсолют с краями по ОБЕИМ сторонам зависимой оси растягивается
        // краями, и отношение обязано победить растяжку (`abspos-005/006`)
        // — ему отношение остаётся в раскладке; без краёв автоминимум
        // действует как у блока (`abspos-012/013`).
        && !(matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed))
            && (if px_w {
                set_len(c.inset.top) && set_len(c.inset.bottom)
            } else {
                set_len(c.inset.left) && set_len(c.inset.right)
            }))
        && (px_w != px_h)
        && (if px_w {
            !matches!(sizes.minimum[1], Some(Len::Px(_)))
        } else {
            !matches!(sizes.minimum[0], Some(Len::Px(_)))
        })
        // Процентный предел зависимой оси эмуляцией не выразить: явный
        // минимум из соотношения сильнее любого максимума (CSS 2.1 §10.4), а
        // доля предела во вкладе должна игнорироваться и решаться только в
        // раскладке (css-sizing-3 §5.2.1; `intrinsic-percent-non-replaced-007`:
        // `height:100px; 2/1; max-width:50%` в `max-content` — 100×100, а не
        // 200×100). Такой коробке соотношение остаётся в раскладке.
        && (if px_w {
            !matches!(sizes.maximum[1], Some(Len::Pct(_)))
        } else {
            !matches!(sizes.maximum[0], Some(Len::Pct(_)))
        })
}
