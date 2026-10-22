//! Значения длинных свойств многоколонника и линеек промежутков реестра
//! (css-multicol-1 §3, css-multicol-2, css-gaps-1 §3-§6): ОДИН разбор на
//! свойство — им реестр проверяет объявление, им же обработчик пишет поля
//! стиля.

use super::{gap_color, gap_inset, gap_list, gap_style, gap_width};
use crate::document::state::ValueCx;
use crate::style::css::LonghandId;
use crate::style::values::value::Len;

/// Годится ли значение длинному свойству многоколонника или линейки.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    let v = value.trim();
    match id {
        ColumnWidth => column_width(v, vcx).is_some(),
        ColumnCount => column_count(v).is_some(),
        ColumnHeight => column_height(v, vcx).is_some(),
        ColumnWrap => column_wrap(v).is_some(),
        ColumnRuleWidth | RowRuleWidth => gap_list(v, |s| gap_width(s, vcx)).is_some(),
        ColumnRuleStyle | RowRuleStyle => gap_list(v, gap_style).is_some(),
        ColumnRuleColor | RowRuleColor => gap_list(v, |s| gap_color(s, vcx)).is_some(),
        ColumnRuleBreak | RowRuleBreak => rule_break(v).is_some(),
        ColumnRuleVisibilityItems | RowRuleVisibilityItems => visibility_items(v).is_some(),
        ColumnRuleInsetCapStart
        | ColumnRuleInsetCapEnd
        | ColumnRuleInsetJunctionStart
        | ColumnRuleInsetJunctionEnd
        | RowRuleInsetCapStart
        | RowRuleInsetCapEnd
        | RowRuleInsetJunctionStart
        | RowRuleInsetJunctionEnd => gap_inset(v, vcx).is_some(),
        _ => false,
    }
}

/// Длина колонки без долей и слов размера (`<length>`).
fn plain_length(v: &str, vcx: ValueCx<'_>) -> Option<Len> {
    Len::parse(v, vcx).filter(|l| {
        !matches!(
            l,
            Len::Auto | Len::Pct(_) | Len::MinContent | Len::MaxContent | Len::FitContent
        )
    })
}

/// `column-width: auto | <length [0,∞]>` (css-multicol-1 §3.1); `None`
/// внутри — `auto`. Нулевая ширина колонке не задаётся (раскладка числа
/// колонок делит на неё) — прежнее ограничение обработчика.
pub(crate) fn column_width(v: &str, vcx: ValueCx<'_>) -> Option<Option<Len>> {
    if v.trim().eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    if crate::style::values::value::number(v, vcx).is_some_and(|n| n != 0.0) {
        return None;
    }
    plain_length(v.trim(), vcx)
        .filter(|l| !matches!(l, Len::Px(w) | Len::Em(w) if *w <= 0.0))
        .map(Some)
}

/// `column-count: auto | <integer [1,∞]>` (css-multicol-1 §3.2).
pub(crate) fn column_count(v: &str) -> Option<Option<u16>> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    v.parse::<u16>().ok().filter(|n| *n > 0).map(Some)
}

/// `column-height: auto | <length [0,∞]>` (css-multicol-2 §ch); ноль —
/// законная высота (`columns: 2 / 0`).
pub(crate) fn column_height(v: &str, vcx: ValueCx<'_>) -> Option<Option<Len>> {
    if v.trim().eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    if crate::style::values::value::number(v, vcx).is_some_and(|n| n != 0.0) {
        return None;
    }
    plain_length(v.trim(), vcx)
        .filter(|l| !matches!(l, Len::Px(h) | Len::Em(h) if *h < 0.0))
        .map(Some)
}

/// `column-wrap: auto | nowrap | wrap` (css-multicol-2 §cwr); `None` —
/// `auto`.
pub(crate) fn column_wrap(v: &str) -> Option<Option<bool>> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "auto" => None,
        "wrap" => Some(true),
        "nowrap" => Some(false),
        _ => return None,
    })
}

/// `*-rule-break: none | normal | intersection` (css-gaps-1 §break): 0, 1, 2.
pub(crate) fn rule_break(v: &str) -> Option<u8> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "none" => 0,
        "normal" => 1,
        "intersection" => 2,
        _ => return None,
    })
}

/// `*-rule-visibility-items: all | around | between | normal`
/// (css-gaps-1 §visibility-items): 0 normal, 1 all, 2 around, 3 between.
pub(crate) fn visibility_items(v: &str) -> Option<u8> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "normal" => 0,
        "all" => 1,
        "around" => 2,
        "between" => 3,
        _ => return None,
    })
}
