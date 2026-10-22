//! Значения длинных свойств текста реестра (css-text-4 §3, §5.1;
//! css-text-decor-4 §2-§3): ОДИН разбор на свойство — им реестр проверяет
//! объявление, им же обработчик пишет поля стиля.

use crate::document::state::ValueCx;
use crate::style::computed::{DecorLen, DecorStyle, parse_decor_style, parse_decor_thickness};
use crate::style::css::LonghandId;
use crate::style::values::value::StyleColor;

/// Годится ли значение длинному свойству текста или оформления.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    let v = value.trim();
    match id {
        WhiteSpaceCollapse => collapse(v).is_some(),
        TextWrapMode => wrap_mode(v).is_some(),
        TextWrapStyle => wrap_style(v).is_some(),
        TextEmphasisStyle => emphasis_style(v).is_some(),
        TextEmphasisColor | TextDecorationColor => StyleColor::parse_value(v, vcx).is_some(),
        TextDecorationLine => decoration_line(v).is_some(),
        TextDecorationStyle => decoration_style(v).is_some(),
        TextDecorationThickness => decoration_thickness(v, vcx).is_some(),
        _ => false,
    }
}

/// Схлопывание пробелов: сохранять пробелы, сохранять переводы строк,
/// переносить после пробелов.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Collapse {
    pub keep_spaces: bool,
    pub preserve_newlines: bool,
    pub break_after_spaces: bool,
}

/// `white-space-collapse: collapse | preserve | preserve-breaks | break-spaces`
/// (css-text-4 §3.1). `discard` и `preserve-spaces` сборка текста не умеет —
/// как и Blink (`css_properties.json5` `white-space-collapse`), они негодны.
pub(crate) fn collapse(v: &str) -> Option<Collapse> {
    let (keep_spaces, preserve_newlines, break_after_spaces) =
        match v.trim().to_ascii_lowercase().as_str() {
            "collapse" => (false, false, false),
            "preserve" => (true, true, false),
            "preserve-breaks" => (false, true, false),
            "break-spaces" => (true, true, true),
            _ => return None,
        };
    Some(Collapse {
        keep_spaces,
        preserve_newlines,
        break_after_spaces,
    })
}

/// `text-wrap-mode: wrap | nowrap` (css-text-4 §5.1): `true` — без переноса.
pub(crate) fn wrap_mode(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "wrap" => Some(false),
        "nowrap" => Some(true),
        _ => None,
    }
}

/// `text-wrap-style: auto | balance | stable | pretty | avoid-short-last-line`
/// (css-text-4 §5.2): `true` — выравнивать длины строк.
pub(crate) fn wrap_style(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "balance" => Some(true),
        "auto" | "stable" | "pretty" | "avoid-short-last-line" => Some(false),
        _ => None,
    }
}

/// Знак акцента: текст знака (пустой — `none`) и признак формы по умолчанию
/// (решается письмом элемента).
pub(crate) fn emphasis_style(v: &str) -> Option<(String, bool)> {
    let v = v.trim();
    // `<string>`: берётся первый знак (css-text-decor-4 §3.1).
    if let Some(q) = v.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        if v.len() < 2 || !v.ends_with(q) {
            return None;
        }
        let body = crate::style::computed::parse_util::unescape_content(&v[1..v.len() - 1]);
        return Some((
            body.chars().next().map(String::from).unwrap_or_default(),
            false,
        ));
    }
    let lower = v.to_ascii_lowercase();
    if lower == "none" {
        return Some((String::new(), false));
    }
    // `[ filled | open ] || [ dot | circle | double-circle | triangle | sesame ]`.
    let (mut fill, mut shape) = (None, None);
    for word in lower.split_whitespace() {
        match word {
            "filled" | "open" if fill.is_none() => fill = Some(word == "open"),
            "dot" | "circle" | "double-circle" | "triangle" | "sesame" if shape.is_none() => {
                shape = Some(word)
            }
            _ => return None,
        }
    }
    fill.or(shape.map(|_| false))?;
    let open = fill.unwrap_or(false);
    let mark = match (shape.unwrap_or("circle"), open) {
        ("dot", false) => '\u{2022}',
        ("dot", true) => '\u{25E6}',
        ("circle", false) => '\u{25CF}',
        ("circle", true) => '\u{25CB}',
        ("double-circle", false) => '\u{25C9}',
        ("double-circle", true) => '\u{25CE}',
        ("triangle", false) => '\u{25B2}',
        ("triangle", true) => '\u{25B3}',
        ("sesame", false) => '\u{FE45}',
        _ => '\u{FE46}',
    };
    Some((mark.to_string(), shape.is_none()))
}

/// `text-decoration-line: none | [ underline || overline || line-through ||
/// blink ] | spelling-error | grammar-error` (css-text-decor-4 §2.1): разряды
/// линий. Мигание и пометки правописания линий не рисуют.
pub(crate) fn decoration_line(v: &str) -> Option<u8> {
    use crate::style::computed::{DECOR_OVER, DECOR_THROUGH, DECOR_UNDER};
    let lower = v.trim().to_ascii_lowercase();
    if matches!(lower.as_str(), "none" | "spelling-error" | "grammar-error") {
        return Some(0);
    }
    let (mut lines, mut blink) = (0u8, false);
    for word in lower.split_whitespace() {
        let bit = match word {
            "underline" => DECOR_UNDER,
            "overline" => DECOR_OVER,
            "line-through" => DECOR_THROUGH,
            "blink" if !blink => {
                blink = true;
                continue;
            }
            _ => return None,
        };
        if lines & bit != 0 {
            return None;
        }
        lines |= bit;
    }
    (lines != 0 || blink).then_some(lines)
}

/// `text-decoration-style: solid | double | dotted | dashed | wavy`.
pub(crate) fn decoration_style(v: &str) -> Option<DecorStyle> {
    parse_decor_style(&v.trim().to_ascii_lowercase())
}

/// `text-decoration-thickness: auto | from-font | <length-percentage>`.
pub(crate) fn decoration_thickness(v: &str, vcx: ValueCx<'_>) -> Option<DecorLen> {
    parse_decor_thickness(&v.trim().to_ascii_lowercase(), vcx)
}
