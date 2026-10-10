//! Пробелы: обрезка краёв, схлопывание, висящие хвосты, принудительные разрывы, широкие знаки.

mod line_breaks;
pub(super) use crate::text::inline::whitespace::line_breaks::blank_line_break;
pub(super) use crate::text::inline::whitespace::line_breaks::line_break;
use crate::text::inline::whitespace::line_breaks::opening_punct;
use crate::text::inline::whitespace::line_breaks::wide_cjk;
use crate::text::inline::whitespace::line_breaks::wide_punct;

use crate::text::inline::*;

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v158→v162, `scout-trimedge-2026-09.md` Х1):
/// узкий гейт «атом обрывает ВЕДУЩИЙ срез, если за рядом пробелов текст»
/// (`trim_edge` на срезе + `text_after_spaces`). Обещание 0/0 с восьмёркой
/// контроля — восьмёрка устояла, но `baseline-inline-non-replaced-004`
/// 0.40 → «красное видно»; без гейта (v162) снова 0.40. Плюсов ноль. Пробел
/// за атомом остаётся прозрачным; эталоны `text-emphasis` по-прежнему ждут
/// другой раскладки — искать её в `as_wrapped_row`, а не в срезе краёв.
/// Пробелы по КРАЯМ строки, сквозь ПУСТУЮ строчную коробку.
///
/// По css-text-3 §4.1.3 схлопываемые пробелы в конце строки удаляются. Пустая
/// строчная коробка (`<span style="border-left:30px solid green"></span>`)
/// ряд пробелов не разрывает: пробелы по обе её стороны — один ряд, он
/// последний в строке и потому исчезает весь. Наш абзац к этому моменту уже
/// разложен на куски, и коробка стоит между ними отдельным куском — поэтому
/// проход идёт с конца и коробки пропускает
/// (`line-edge-white-space-collapse-001` и `-002`: иначе у рамки оставался
/// лишний пробел и из-под неё выглядывало красное).
pub fn trim_edge_spaces(pieces: &mut [Piece]) {
    trim_edge(pieces.iter_mut().rev(), false, false);
    trim_edge(pieces.iter_mut(), true, false);
}

/// То же для абзаца, чьи атомы встают В СТРОКУ (`Paragraph::atoms`): там атом
/// — содержимое строки (CSS 2.1 §9.2.2), и ряд пробелов ЗА ним уже не на краю.
/// Прозрачный атом срезал пробелы между атомами: `<img> <img>` слипались.
pub fn trim_edge_spaces_solid_atoms(pieces: &mut [Piece]) {
    trim_edge(pieces.iter_mut().rev(), false, true);
    trim_edge(pieces.iter_mut(), true, true);
}

/// Один край строки: куски идут от него внутрь, коробки пропускаются.
fn trim_edge<'a>(pieces: impl Iterator<Item = &'a mut Piece>, leading: bool, solid_atoms: bool) {
    for piece in pieces {
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v155, `scout-emphasis-2026-09.md`):
        // отрисовка `text-emphasis` (11 хунков) вместе с правкой `trim_edge`
        // (`Piece::Atom` перестаёт быть прозрачным для среза краевого пробела).
        // Срез css-text-decor+css-pseudo+css-lists+css-counter-styles+css-ruby+
        // css-text+css-inline+CSS2 8145: +4/−8 у этой части —
        // `inline-block-baseline-015/016` 0.00 → 99.00,
        // `vertical-align-117a/118a` 0.11 → 6.84, `inline-formatting-context-013`,
        // `line-breaking-030/032`, `inline-block-replaced-width-003`.
        // Срез краевого пробела после атома трогает всю строчную раскладку —
        // мерить отдельно и сначала только его.
        match piece {
            Piece::Atom(_) if solid_atoms => return,
            // Коробка без текста для ряда пробелов прозрачна.
            Piece::Atom(_) | Piece::Overlay(..) => continue,
            // Распорка полей строчной коробки и метка атома прозрачны так же:
            // место они занимают, содержимым строки не являются, и пробел за
            // ними по-прежнему стоит на КРАЮ строки (css-text-3 §4.1.3).
            // Прежде первая же распорка обрывала проход, и ведущий пробел
            // после `<span style="padding-left:1em">` не срезался никогда.
            Piece::Text { text, .. } if text == SPACER || text == ZWSP => continue,
            Piece::Text { text, style } => {
                // `white-space: pre*` пробелы бережёт — там удалять нечего.
                if style.keep_spaces == Some(true) {
                    return;
                }
                // Пустой кусок ряда не обрывает: он и есть схлопнутый пробел,
                // а настоящий край строки лежит дальше внутрь.
                if text.is_empty() {
                    continue;
                }
                let trimmed = if leading {
                    text.trim_start_matches(' ')
                } else {
                    text.trim_end_matches(' ')
                };
                if trimmed.len() == text.len() {
                    // Кусок упирается не в пробел: ряд оборвался, дальше не идём.
                    return;
                }
                let rest = trimmed.to_string();
                let empty = rest.is_empty();
                *text = rest;
                if !empty {
                    return;
                }
                // Кусок был из одних пробелов — ряд продолжается левее.
            }
        }
    }
}

/// Отрезать от текст-куска перед атомом хвост из открывающих знаков.
///
/// Перенос после открывающей скобки запрещён (UAX #14): когда за текстом
/// идёт строчный атом (`text-combine-upright`, картинка), скобка обязана
/// уйти на строку ВМЕСТЕ с ним. Внутри одного текст-куска это делает
/// перенос строк, но границу куска он не видит — скобка застревала
/// последней строкой текста, а атом падал на следующую
/// (text-combine-upright-line-breaking-rules-001).
pub(super) fn split_glued_tail(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len());
    let mut it = pieces.into_iter().peekable();
    while let Some(p) = it.next() {
        match p {
            Piece::Text { text, style }
                if matches!(it.peek(), Some(Piece::Atom(_)))
                    && text.chars().next_back().is_some_and(opening_punct) =>
            {
                let head_len = text.trim_end_matches(opening_punct).len();
                let tail = text[head_len..].to_string();
                if head_len > 0 {
                    out.push(Piece::Text {
                        text: text[..head_len].to_string(),
                        style: style.clone(),
                    });
                }
                out.push(Piece::Text { text: tail, style });
            }
            p => out.push(p),
        }
    }
    out
}

pub(super) fn drop_hanging_tail(mut pieces: Vec<Piece>) -> Vec<Piece> {
    while let Some(Piece::Text { text, style }) = pieces.last() {
        // Схлопываемый пробел по CSS — только `space`, `tab`, `CR`, `LF`.
        // Идеографический U+3000 и неразрывный U+00A0 значимы: `trim()` их
        // тоже снимает, и хвостовая строка из них пропадала целиком
        // (`trailing-ideographic-space-017`).
        let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
        if style.keep_spaces == Some(true) || !text.chars().all(collapsible) {
            break;
        }
        pieces.pop();
    }
    pieces
}

/// Схлопывание пробелов, как в HTML: переводы строк и повторы — один пробел.
pub(super) fn normalize_spaces(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut at = 0usize;
    while at < chars.len() {
        if !is_collapsible(chars[at]) {
            out.push(chars[at]);
            at += 1;
            continue;
        }
        // Пробельный отрезок целиком: важно, был ли внутри перевод строки и
        // какие знаки стоят по краям.
        let start = at;
        while at < chars.len() && is_collapsible(chars[at]) {
            at += 1;
        }
        let had_break = chars[start..at].iter().any(|c| matches!(*c, '\n' | '\r'));
        // Соседи ряда ищутся СКВОЗЬ знаки управления двунаправленностью
        // (css-text-3 §4.1: «as if they were not there»): ряд по ту сторону
        // RLO/PDF — продолжение прежнего и удаляется целиком (CSS 2.1 §16.6.1
        // шаг 4), `x ␠RLO␠x` — один пробел (`white-space-collapsing-bidi-001`).
        let before = out.chars().rev().find(|c| !bidi_format(*c));
        if before == Some(' ') {
            continue;
        }
        let after = chars[at..].iter().copied().find(|c| !bidi_format(*c));
        // Преобразование перевода строки (CSS Text 3 §4.1.2): между двумя
        // ШИРОКИМИ знаками перевод УДАЛЯЕТСЯ, а не становится пробелом —
        // иначе японский текст, набранный в несколько строк, получает лишние
        // пробелы на каждом переводе.
        // Перевод строки рядом с нулевым пробелом удаляется, нулевой пробел
        // остаётся (css-text-4 §4.1.3 «If the character immediately before or
        // immediately after the segment break is the zero-width space
        // character (U+200B), then the break is removed»).
        // Широкий ЗНАК ПРЕПИНАНИЯ с любой стороны — тоже удаление (Gecko,
        // bug 1935148, `segment-break-transformation-punctuation-001`:
        // «場合、⏎Edge» и «ID⏎｢smith｣» без пробела).
        let drop = had_break
            && ((before.is_some_and(wide_cjk) && after.is_some_and(wide_cjk))
                || before.is_some_and(wide_punct)
                || after.is_some_and(wide_punct)
                || before == Some('\u{200b}')
                || after == Some('\u{200b}'));
        if !drop {
            out.push(' ');
        }
    }
    out
}

// Табуляция до ближайшей ПОЗИЦИИ табуляции, а не в `tab-size` пробелов.
//
// По CSS `tab-size: 8` значит, что табуляция доводит строку до ближайшего
// кратного восьми, то есть от третьего знака добирает пять пробелов, а не
// восемь. Пока раскрывалось постоянным числом, отступ кода после любого

/// Схлопываемый пробел по CSS — ТОЛЬКО эти четыре знака.
///
/// Остальные пробельные символы юникода — обычные знаки со своей шириной:
/// идеографический пробел `U+3000` держит место целого иероглифа, неразрывный
/// `U+00A0` не даёт разорвать строку. Пока схлопывалось всё пробельное подряд,
/// такой пробел пропадал из текста вместе со своей шириной.
/// Знак управления двунаправленностью — встраивание, отмена, изоляция
/// (UAX #9: LRE/RLE/PDF/LRO/RLO, LRI/RLI/FSI/PDI). Для обработки пробелов
/// его нет вовсе: css-text-3 §4.1 — «ignoring bidi formatting characters as
/// if they were not there» (`white-space-collapsing-bidi-001/002`).
pub(super) fn bidi_format(ch: char) -> bool {
    matches!(ch as u32, 0x202A..=0x202E | 0x2066..=0x2069)
}

pub(super) fn is_collapsible(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}
