//! Collapse for inline; split out to keep the owning module within 250 lines.

use super::Piece;
use crate::style::values::value::Len;
use crate::text::inline::spacers::*;
use crate::text::inline::whitespace::*;

/// Схлопывание пробелов ЧЕРЕЗ границу кусков (CSS 2.1 §16.6.1,
/// css-text-3 §4.1.1).
///
/// Схлопывание у нас поузловое, а ряд пробелов сплошь и рядом лежит в РАЗНЫХ
/// кусках: спека прямо оговаривает пробел «even one outside the boundary of
/// the inline containing that space». `<span>Row 1, </span>` плюс перевод
/// строки перед следующим тегом набирались как два пробела — на знак шире
/// эталона, и так на каждой границе.
///
/// Кусок из ОДНИХ пробелов ряд не разрывает: он схлопывается в предыдущий
/// пробел целиком и остаётся пустым.
///
/// ПРОБОВАЛИ И ОТКАТИЛИ: пропускать здесь и СЛУЖЕБНЫЕ куски — распорку полей
/// строчной коробки (U+FEFF) и метку границы атома (U+200B), — не сбрасывая
/// признак пробела. Замерено по семьям text/*, linebox/*, bidi-*: 0 и 0.
/// Семью `white-space-normal-*` держит не это.
pub fn collapse_across_pieces(pieces: &mut [Piece]) {
    let mut prev_space = false;
    for piece in pieces.iter_mut() {
        match piece {
            // Кусок вне потока места не занимает и ряд пробелов не рвёт.
            Piece::Overlay(..) => {}
            // Замещаемая коробка — не пробел: ряд на ней кончается.
            Piece::Atom(_) => prev_space = false,
            Piece::Text { text, style } => {
                // A zero-advance empty-box metric marker is not document
                // content and must not interrupt adjoining collapsible spaces.
                if text == SPACER && style.letter_spacing == Some(Len::Px(0.0)) {
                    continue;
                }
                // `white-space: pre*` пробелы бережёт — там схлопывать нечего.
                // `pre-line` переводы строк бережёт, а ПРОБЕЛЫ схлопывает
                // (§16.6): освобождать его от схлопки нельзя.
                if style.keep_spaces == Some(true) {
                    prev_space = false;
                    continue;
                }
                // Кусок-метка направления (`bidi_marks`: RLE/LRE/PDF вокруг
                // `<span dir>`) ряд пробелов НЕ рвёт и сам его не начинает —
                // css-text-3 §4.1 велит обрабатывать пробелы, не видя этих
                // знаков. Иначе `x <span dir=rtl> x </span> x` набирался
                // семью знаками вместо пяти (`white-space-collapsing-bidi-002`).
                if !text.is_empty() && text.chars().all(bidi_format) {
                    continue;
                }
                if prev_space {
                    let rest = text.trim_start_matches(' ');
                    if rest.len() != text.len() {
                        *text = rest.to_string();
                    }
                }
                match text.chars().next_back() {
                    Some(' ') => prev_space = true,
                    // Пустой кусок ряда не меняет: он и есть схлопнутый пробел.
                    None => {}
                    Some(_) => prev_space = false,
                }
            }
        }
    }
}
