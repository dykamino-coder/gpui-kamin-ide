//! Line breaks for whitespace; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;
use gpui::{AnyElement, Styled};

/// Убрать ХВОСТОВОЙ пробельный кусок строки-ряда.
///
/// По CSS пробел в конце строки висит за краем и на раскладку не влияет. В
/// ряду он влияет: это отдельная коробка со своей высотой, и строка растёт на
/// его спуск — между двумя картинками 60×60 появлялась полоса в полкегля
/// (`line-breaking-030`). В сохранённых пробелах (`white-space: pre*`) кусок
/// значим и остаётся.
/// Открывающий знак, после которого перенос запрещён (UAX #14, класс OP;
/// CJK-набор). Такой знак клеится к СЛЕДУЮЩЕМУ содержимому.
pub(super) fn opening_punct(c: char) -> bool {
    matches!(
        c,
        '「' | '『'
            | '（'
            | '〔'
            | '【'
            | '〈'
            | '《'
            | '〖'
            | '〘'
            | '〚'
            | '｛'
            | '［'
            | '｟'
            | '｢'
    )
}

/// Разрыв строки в ряду из слов.
///
/// Ряд — гибкая строка с переносом, и перевод строки в нём не значит ничего:
/// `<br>` доезжал сюда куском текста `\n` и молча пропадал. Разрыв даёт
/// распорка во всю ширину — следующему ребёнку места в строке уже нет.
/// Разрыв, закрывающий ПУСТУЮ строку (в ней ни слова, ни атома): такая
/// строка не «zero-height» (CSS 2.1 §9.4.2 исключает только строки без
/// текста и без разрыва), её высоту даёт strut — `line-height` блока
/// (§10.8.1). Распорка `h_0` роняла строку `отступ + <br>` в ноль, и квадрат
/// вставал наверх вместо низа (`text-indent-on-blank-line-rtl-left-align`).
pub(crate) fn blank_line_break(style: &Computed) -> AnyElement {
    let size = match style.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let lh = match style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => size * 1.2,
    };
    gpui::div()
        .w_full()
        .h(gpui::px(lh))
        .flex_shrink_0()
        .into_any_element()
}

pub(crate) fn line_break() -> AnyElement {
    gpui::div().w_full().h_0().into_any_element()
}

/// Широкий знак письма, между которыми перевод строки удаляется.
///
/// Хангыль сюда НЕ входит: по спецификации он пишется через пробелы, и
/// перевод между слогами обязан стать пробелом.
pub(super) fn wide_cjk(ch: char) -> bool {
    let c = ch as u32;
    let hangul = (0x1100..=0x11FF).contains(&c)
        || (0x3130..=0x318F).contains(&c)
        || (0xA960..=0xA97F).contains(&c)
        || (0xAC00..=0xD7FF).contains(&c);
    if hangul {
        return false;
    }
    (0x2E80..=0x303E).contains(&c)
        || (0x3041..=0x33FF).contains(&c)
        || (0x3400..=0x4DBF).contains(&c)
        || (0x4E00..=0x9FFF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0xFE30..=0xFE4F).contains(&c)
        || (0xFF01..=0xFF60).contains(&c)
        // Полуширинная кана и её знаки (East Asian Width H; полуширинный
        // хангыль U+FFA0.. — Hangul, в счёт не идёт).
        || (0xFF61..=0xFF9F).contains(&c)
        || (0xFFE0..=0xFFE6).contains(&c)
        || (0x20000..=0x3FFFD).contains(&c)
}

/// Широкий (F/W/H) знак препинания письма CJK.
pub(super) fn wide_punct(ch: char) -> bool {
    let c = ch as u32;
    (0x3000..=0x303F).contains(&c)
        || c == 0x30A0
        || c == 0x30FB
        || (0xFE30..=0xFE4F).contains(&c)
        || (0xFF01..=0xFF0F).contains(&c)
        || (0xFF1A..=0xFF20).contains(&c)
        || (0xFF3B..=0xFF40).contains(&c)
        || (0xFF5B..=0xFF65).contains(&c)
}
