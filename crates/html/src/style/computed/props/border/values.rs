//! Грамматики значений длинных свойств рамки и контура (`LonghandId::accepts`
//! и применение берут ОДНУ функцию разбора): `<line-width>`, `<line-style>`,
//! цвет стороны, `outline-style`, `outline-color`, `outline-offset` и части
//! `border-image`.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, ParseResult};
use crate::style::css::{Grammar, LonghandId};
use crate::style::values::value::{Color, Len};

/// Годится ли значение длинному свойству рамки или контура.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    match id.grammar() {
        Grammar::LineWidth => line_width(value, vcx).is_some(),
        Grammar::LineStyle | Grammar::OutlineStyle => line_style_ok(id, value),
        Grammar::LineColor => line_color(value, vcx).is_some(),
        Grammar::OutlineColor => outline_color(value, vcx).is_some(),
        Grammar::OutlineOffset => outline_offset(value, vcx).is_some(),
        Grammar::BorderImage => {
            let mut parser = Computed {
                declaration_parse: ParseResult::Invalid,
                ..Computed::default()
            };
            parser.set_border_image(id.name(), value, vcx);
            parser.declaration_parse == ParseResult::Accepted
        }
        _ => false,
    }
}

/// Стиль линии годен свойству `id`: `<line-style>` у `border-*-style`,
/// `auto | <outline-line-style>` у `outline-style`.
pub(crate) fn line_style_ok(id: LonghandId, value: &str) -> bool {
    match id.grammar() {
        Grammar::OutlineStyle => outline_line_style(value).is_some(),
        _ => border_style_rank(value).is_some(),
    }
}

/// `<line-width>` (css-backgrounds-3 §4.3): `thin`, `medium`, `thick` или
/// неотрицательная `<length>`. Доля длиной не бывает, отрицательная длина
/// негодна, а результат `calc()` зажимается в допустимый диапазон
/// (css-values-4 §10.12).
pub(crate) fn line_width(v: &str, vcx: ValueCx<'_>) -> Option<Len> {
    match v.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => match Len::parse(v, vcx)? {
            Len::Pct(_) => None,
            Len::Px(v) | Len::Em(v) | Len::Ex(v) | Len::Ch(v) if v < 0.0 => None,
            l => Some(l),
        },
    }
}

/// Ранг стиля кромки (`<line-style>`, css-backgrounds-3 §4.2) для разбора
/// конфликтов сросшихся рамок (CSS 2.1 §17.6.2.1): 0 none, 1 hidden,
/// 3 inset, 4 groove, 5 outset, 6 ridge, 7 dotted, 8 dashed, 9 solid,
/// 10 double. Рамку рисуют ранги от 3: `none` и `hidden` её убирают.
pub(crate) fn border_style_rank(v: &str) -> Option<u8> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "none" => 0,
        "hidden" => 1,
        "inset" => 3,
        "groove" => 4,
        "outset" => 5,
        "ridge" => 6,
        "dotted" => 7,
        "dashed" => 8,
        "solid" => 9,
        "double" => 10,
        _ => return None,
    })
}

/// Цвет стороны: `Some(None)` — `currentcolor`, его решает цвет текста
/// после наследования.
pub(crate) fn line_color(v: &str, vcx: ValueCx<'_>) -> Option<Option<Color>> {
    if v.trim().eq_ignore_ascii_case("currentcolor") {
        return Some(None);
    }
    Color::parse(v, vcx).map(Some)
}

/// Код рисунка контура (`outline_style`): `hidden` контуру не годится
/// (css-ui-4 §outline-style).
pub(crate) fn outline_line_style(v: &str) -> Option<u8> {
    let lower = v.trim().to_ascii_lowercase();
    if lower == "hidden" {
        return None;
    }
    crate::style::computed::outline_style_of(&lower)
}

/// `outline-color`: `auto` и `currentcolor` хранятся пустым слотом — цвет
/// решает отрисовка (акцентный при `outline-style: auto`, иначе цвет текста).
pub(crate) fn outline_color(v: &str, vcx: ValueCx<'_>) -> Option<Option<Color>> {
    if v.trim().eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    line_color(v, vcx)
}

/// `outline-offset`: `<length>` или `inset` (сдвиг минус толщина) —
/// `(сдвиг, inset)`.
pub(crate) fn outline_offset(v: &str, vcx: ValueCx<'_>) -> Option<(Option<Len>, bool)> {
    if v.trim().eq_ignore_ascii_case("inset") {
        return Some((None, true));
    }
    match Len::parse(v, vcx)? {
        Len::Pct(_) => None,
        l => Some((Some(l), false)),
    }
}
