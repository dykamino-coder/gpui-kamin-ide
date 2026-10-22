//! Кегль и высота строки (css-fonts-4 §font-size-prop, css-inline-3
//! §line-height): разбор, общий для проверки реестром и обработчика.

use crate::document::state::ValueCx;
use crate::style::values::value::Len;

/// Кегль (css-fonts-4 §font-size-prop).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FontSize {
    /// `math` (MathML Core §3.2.2): кегль по `math-depth`.
    Math,
    /// Абсолютное или относительное слово: длина и шаг по таблице
    /// (`larger` +1, `smaller` −1).
    Keyword(Len, i8),
    /// `<length-percentage [0,∞]>`.
    Length(Len),
}

fn negative(l: &Len) -> bool {
    matches!(
        l,
        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v) if *v < 0.0
    )
}

/// `font-size: <absolute-size> | <relative-size> | <length-percentage [0,∞]> |
/// math`. Слова — таблица Chrome при базовом кегле 16 (css-fonts-4
/// §absolute-size).
pub(crate) fn font_size(v: &str, vcx: ValueCx<'_>) -> Option<FontSize> {
    let lower = v.trim().to_ascii_lowercase();
    let px = |p: f32| Some(FontSize::Keyword(Len::Px(p), 0));
    match lower.as_str() {
        "math" => Some(FontSize::Math),
        "xx-small" => px(9.0),
        "x-small" => px(10.0),
        "small" => px(13.0),
        "medium" => px(16.0),
        "large" => px(18.0),
        "x-large" => px(24.0),
        "xx-large" => px(32.0),
        "xxx-large" => px(48.0),
        // Относительные — доли кегля РОДИТЕЛЯ; шаг по таблице решает
        // наследование (`font-size-121`).
        "smaller" => Some(FontSize::Keyword(Len::Em(5.0 / 6.0), -1)),
        "larger" => Some(FontSize::Keyword(Len::Em(1.2), 1)),
        _ => match Len::parse_mixed(v.trim(), vcx) {
            Some(l) if !negative(&l) && !matches!(l, Len::Auto) => Some(FontSize::Length(l)),
            _ => None,
        },
    }
}

/// `line-height: normal | <number [0,∞]> | <length-percentage [0,∞]>`
/// (css-inline-3 §line-height). Число — множитель (`Len::Pct`), доля — от
/// своего кегля (`Len::Em`, наследуется точками, CSS 2.1 §10.8.1),
/// `normal` — метка `Len::Auto`.
pub(crate) fn line_height(v: &str, vcx: ValueCx<'_>) -> Option<Len> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("normal") {
        return Some(Len::Auto);
    }
    let parsed = match v.parse::<f32>() {
        Ok(mult) if mult.is_finite() => Len::Pct(mult),
        _ => match Len::parse(v, vcx)? {
            Len::Pct(k) if v.ends_with('%') => Len::Em(k),
            Len::Auto => return None,
            other => other,
        },
    };
    (!negative(&parsed)).then_some(parsed)
}
