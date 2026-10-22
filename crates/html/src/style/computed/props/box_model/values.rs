//! Грамматики значений длинных свойств box-реестра (`LonghandId::accepts`):
//! стороны полей, отступов и вставок и размеры коробки. Проверка разбора и
//! применение берут ОДНУ функцию разбора, поэтому годное при разборе
//! объявление применяется целиком, а негодное в каскад не попадает.

use crate::document::state::ValueCx;
use crate::style::computed::size_range::parse_nonnegative_size;
use crate::style::computed::*;
use crate::style::css::{Grammar, LonghandId, Slot};
use crate::style::values::value::{Len, fold_zero_percentage};

impl Computed {
    /// Годится ли значение длинному свойству реестра (без общих слов и
    /// подстановок — их разбирает каскад).
    pub(crate) fn accepts_longhand(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
        match (id.grammar(), id.slot()) {
            (Grammar::Size, Slot::Physical(_, i)) => physical_size_ok(i, value, vcx),
            (Grammar::Size, Slot::Logical(_, i)) => logical_size_ok(i, value, vcx),
            (Grammar::Margin | Grammar::Padding | Grammar::Inset, _) => {
                side_len(id.grammar(), value, vcx).is_some()
            }
            _ => crate::style::computed::props::border::values::accepts(id, value, vcx),
        }
    }
}

/// Значение стороны: поле (`<length-percentage> | auto | <anchor-size()>`,
/// css-box-4 §3.1, css-anchor-position-1 §5.1), отступ
/// (`<length-percentage [0,∞]>`, css-box-4 §4.1) или вставка (`auto |
/// <length-percentage> | <anchor()> | <anchor-size()>`, css-position-3
/// §3.1). Смесь «доля ± точки» в `calc()` доживает индексом (css-values-4
/// §10.9): долю решает раскладка от строчного размера содержащего блока.
pub(crate) fn side_len(grammar: Grammar, value: &str, vcx: ValueCx<'_>) -> Option<Len> {
    let lower = value.trim().to_ascii_lowercase();
    // Ключевые слова размеров (css-sizing-3 §3.2) — не длины стороны: общий
    // разбор длины их понимает ради `width`.
    if lower.starts_with("fit-content")
        || matches!(
            lower.as_str(),
            "min-content" | "max-content" | "stretch" | "-webkit-fill-available" | "-moz-available"
        )
    {
        return None;
    }
    let mixed = || Len::parse(value, vcx).or_else(|| Len::parse_mixed(value, vcx));
    match grammar {
        Grammar::Padding => {
            if lower == "auto" || lower.contains("anchor") {
                return None;
            }
            // Отрицательная длина негодна, а результат `calc()` зажимается в
            // допустимый диапазон (css-values-4 §10.12).
            fold_zero_percentage(parse_nonnegative_size(value, vcx), vcx)
        }
        // `anchor()` годится только вставке (css-anchor-position-1 §3.2).
        Grammar::Margin if lower.contains("anchor(") => None,
        Grammar::Margin => fold_zero_percentage(mixed(), vcx),
        Grammar::Inset => mixed(),
        _ => None,
    }
}

/// Физический размер (индекс `width, height, min-width, min-height,
/// max-width, max-height`): длина-доля не меньше нуля, ключевые слова
/// размеров, `fit-content(<length-percentage>)`, `calc-size()` (css-sizing-3
/// §3.1-3.2, css-sizing-4 §4.1, css-values-5 §calc-size); `none` — только
/// у пределов `max-*`.
fn physical_size_ok(i: usize, value: &str, vcx: ValueCx<'_>) -> bool {
    if value.trim().eq_ignore_ascii_case("none") {
        return i >= 4;
    }
    let mut parsed = None;
    assign_size(&mut parsed, value, vcx);
    parsed.is_some()
        || calc_size_arg(value).is_some()
        || stretch_keyword(value)
        || fit_content_arg(value, vcx).is_some()
}

/// Логический размер (индекс `inline, block, min-inline, min-block,
/// max-inline, max-block`): то, что понимает его разбор (`assign_size`).
fn logical_size_ok(i: usize, value: &str, vcx: ValueCx<'_>) -> bool {
    if value.trim().eq_ignore_ascii_case("none") {
        return i >= 4;
    }
    let mut parsed = None;
    assign_size(&mut parsed, value, vcx);
    parsed.is_some()
}
