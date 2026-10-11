//! Геометрия строк: выравнивание, точка знака, коробка строчного блока, rtl-края.

mod inline_rect;
mod rtl_extent;

use crate::text::paragraph::*;
use gpui::{Pixels, px};

/// Правила переноса из стиля — и признак, нужна ли своя раскладка вовсе.
///
/// Пока своя раскладка не умеет выделение мышью, поэтому обычный текст
/// остаётся на выделяемом элементе движка. Сюда уходит только то, что иначе
/// не выразить.
/// Правила переноса из стиля.
///
/// Своя раскладка считает ВЕСЬ текст: перенос, выключка и свисающая
/// пунктуация должны решаться одним алгоритмом, иначе соседние абзацы одной
/// страницы ломаются по-разному. Поэтому правила есть всегда — отбор «кому
/// своя раскладка нужна, а кому нет» отсюда снят.
pub fn rules(c: &crate::style::computed::Computed) -> Option<Wrap> {
    Some(wrap_of(c))
}

/// Сдвиг строки вдоль коробки по `text-align` — с УЧЁТОМ ЗНАКА остатка.
///
/// Дословный перенос Blink `length_utils.cc:1607 LineOffsetForTextAlign`.
/// Смысл в том, что обрезание отрицательного остатка зависит от СТОРОНЫ
/// ПИСЬМА БЛОКА, а не от значения `text-align`:
///
/// * ltr — отрицательный остаток гасится всегда: «Wide lines spill out of the
///   block based off direction. So even if text-align is right, if direction
///   is LTR, wide lines should overflow out of the right side of the block»
///   (`length_utils.cc:1634-1636`);
/// * rtl — не гасится никогда: «The direction of the block should determine
///   what happens with wide lines. In particular with RTL blocks, wide lines
///   should still spill out to the left» (`length_utils.cc:1620-1622`).
///
/// По спеке это css-text-4 §7.1 (`right` — «Inline-level content is aligned to
/// the line-right edge of the line box», без оговорки на переполнение) вместе
/// с CSS 2.1 §16.2 (начальное значение `text-align` в rtl действует как
/// `right`) и §9.4.2 («then the inline box overflows the line box»).
///
/// `Justify` сюда не заходит: раздача остатка идёт своим путём и берёт
/// остаток УЖЕ обрезанным — растягивать переполненную строку нечем.
pub(super) fn line_offset(align: Align, rtl: bool, free: Pixels) -> Pixels {
    let zero = px(0.);
    match align {
        Align::Right if rtl => free,
        Align::Right => free.max(zero),
        Align::Left if rtl => free.min(zero),
        Align::Left => zero,
        // При rtl и положительном остатке — та же половина, что и при ltr;
        // при отрицательном строка держится правого края целиком.
        Align::Center if rtl && free <= zero => free,
        Align::Center => (free / 2.).max(zero),
        Align::Justify => zero,
    }
}

/// Выключка из стиля.
pub fn align_of(a: Option<crate::style::computed::TextAlign>) -> Align {
    a.map(align_of_value).unwrap_or(Align::Left)
}

/// Выключка абзаца с разворотом логических краёв по стороне письма.
pub fn align_for(c: &crate::style::computed::Computed) -> Align {
    let rtl = c.rtl == Some(true);
    let value = c
        .text_align
        .unwrap_or(crate::style::computed::TextAlign::Start)
        .physical(rtl);
    let align = align_of_value(value);
    // `text-justify: none` — растягивать запрещено, и строка идёт к началу:
    // у письма справа налево началом служит правый край.
    if align == Align::Justify && c.no_justify == Some(true) {
        return if rtl { Align::Right } else { Align::Left };
    }
    align
}

/// Выключка из заданного значения.
pub fn align_of_value(a: crate::style::computed::TextAlign) -> Align {
    match a {
        crate::style::computed::TextAlign::Center => Align::Center,
        crate::style::computed::TextAlign::Right => Align::Right,
        crate::style::computed::TextAlign::Justify => Align::Justify,
        _ => Align::Left,
    }
}
