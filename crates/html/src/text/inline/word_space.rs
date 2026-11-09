//! `word-space-transform` по кускам абзаца (css-text-4 §word-space-transform).

use crate::style::computed::Computed;
use crate::text::inline::*;

/// Пробелы куска набираются U+3000 (`text-transform: full-width`).
fn full_width_spaces(style: &Computed) -> bool {
    style.inh.text_flow.text_transform_flags & crate::style::computed::TT_FULL_WIDTH != 0
}

/// `word-space-transform` по КУСКАМ абзаца.
///
/// Соседи точки переноса сплошь и рядом лежат в других кусках: `あ<wbr>い` —
/// это три куска, а `<span>` с полем или рамкой делит абзац и вовсе на
/// отдельные элементы. Замена знак-в-знак: нулевой пробел и идеографический
/// занимают в UTF-8 одинаково, поэтому прогоны не съезжают.
pub fn space_transform_pieces(pieces: &mut [Piece]) {
    // Правку даёт только U+200B куска, у которого свойство задано; без такого
    // знака посимвольная карта абзаца (24 байта на знак) не строится вовсе.
    if !pieces.iter().any(|piece| {
        matches!(piece, Piece::Text { text, style }
            if style.inh.text_flow.word_space_char.is_some_and(|c| c != '\0')
                && text.contains('\u{200b}'))
    }) {
        return;
    }
    let chars = pieces.iter().map(|piece| match piece {
        Piece::Text { text, .. } => text.chars().count(),
        _ => 1,
    });
    let mut seq: Vec<(usize, usize, char)> = Vec::with_capacity(chars.sum());
    for (i, piece) in pieces.iter().enumerate() {
        match piece {
            Piece::Text { text, style } => {
                // `text-transform: full-width` уже превратил пробел куска в
                // U+3000 (регистр меняется при сборе), а по порядку
                // css-text-3 §2.1 он идёт ПОСЛЕ обработки пробелов — для
                // схлопывания это всё ещё пробел (`word-space-transform-009`).
                let wide = full_width_spaces(style);
                for (at, ch) in text.char_indices() {
                    let ch = if wide && ch == '\u{3000}' { ' ' } else { ch };
                    seq.push((i, at, ch));
                }
            }
            // Атомарная коробка иероглифом не является и соседство разрывает.
            _ => seq.push((usize::MAX, 0, '\u{0}')),
        }
    }
    let mut edits: Vec<(usize, usize, char)> = vec![];
    for k in 0..seq.len() {
        let (piece, at, ch) = seq[k];
        if ch != '\u{200b}' || piece == usize::MAX {
            continue;
        }
        let Piece::Text { style, .. } = &pieces[piece] else {
            continue;
        };
        // Значение `space` подставляет ОБЫЧНЫЙ пробел, и оно тоже работает:
        // сравнение шло только с идеографическим, и половина свойства не
        // действовала вовсе.
        let Some(sep) = style.inh.text_flow.word_space_char.filter(|&c| c != '\0') else {
            continue;
        };
        // `space` — разделитель слов ЛЮБОЙ письменности (css-text-4
        // §word-space-transform: «word separators … are replaced with
        // U+0020»): латинское `aa<wbr>bb` тоже получает пробел
        // (`word-space-transform-014`). Соседи-иероглифы нужны только
        // идеографическому.
        let between = k > 0 && k + 1 < seq.len();
        if between && (sep == ' ' || (ideographic(seq[k - 1].2) && ideographic(seq[k + 1].2))) {
            edits.push((piece, at, sep));
            if sep == ' ' {
                seq[k].2 = sep;
            }
        }
    }
    // Подставленный U+0020 — обычный схлопываемый пробел: замена идёт ДО
    // обработки пробелов (css-text-4 §word-space-transform), и в
    // `i <wbr> &#x200B; j` от серии пробелов остаётся один
    // (`word-space-transform-007`). Схлопываем только серии, где есть
    // подставленный пробел: прочие уже свёрнуты сбором текста.
    let collapsible = |i: usize| {
        i != usize::MAX
            && matches!(&pieces[i], Piece::Text { style, .. } if style.inh.text_flow.keep_spaces != Some(true))
    };
    let mut k = 0;
    while k < seq.len() {
        if seq[k].2 != ' ' || !collapsible(seq[k].0) {
            k += 1;
            continue;
        }
        let mut end = k + 1;
        while end < seq.len() && seq[end].2 == ' ' && collapsible(seq[end].0) {
            end += 1;
        }
        let transformed = |j: usize| edits.iter().any(|e| e.0 == seq[j].0 && e.1 == seq[j].1);
        if (k..end).any(transformed) {
            for j in k + 1..end {
                let (piece, at, _) = seq[j];
                match edits.iter_mut().find(|e| e.0 == piece && e.1 == at) {
                    Some(e) => e.2 = '\0',
                    None => edits.push((piece, at, '\0')),
                }
            }
        }
        k = end;
    }
    // С конца: обычный пробел короче нулевого (1 байт против 3), и правка
    // впереди сдвигала бы смещения следующих правок того же куска.
    edits.sort_by_key(|e| std::cmp::Reverse((e.0, e.1)));
    for (piece, at, sep) in edits {
        if let Piece::Text { text, style } = &mut pieces[piece] {
            let Some(old) = text[at..].chars().next() else {
                continue;
            };
            let sep = if sep == ' ' && full_width_spaces(style) {
                '\u{3000}'
            } else {
                sep
            };
            let mut buf = [0u8; 4];
            let new = if sep == '\0' {
                ""
            } else {
                &*sep.encode_utf8(&mut buf)
            };
            text.replace_range(at..at + old.len_utf8(), new);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::BoxStyle;

    fn piece(text: &str, sep: Option<char>) -> Piece {
        let mut style = Computed::default();
        style.inh.text_flow.word_space_char = sep;
        Piece::Text {
            text: text.into(),
            style: BoxStyle::from(style),
        }
    }

    fn texts(pieces: &[Piece]) -> Vec<String> {
        pieces
            .iter()
            .map(|p| match p {
                Piece::Text { text, .. } => text.to_string(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn zero_width_space_takes_the_separator() {
        let mut pieces = [piece("aa\u{200b}", Some(' ')), piece("bb", Some(' '))];
        space_transform_pieces(&mut pieces);
        assert_eq!(texts(&pieces), ["aa ", "bb"]);
    }

    #[test]
    fn pieces_without_a_transformable_zero_width_space_stay_as_they_are() {
        for mut pieces in [
            [piece("aa bb", Some(' ')), piece("cc", Some(' '))],
            [piece("aa\u{200b}bb", None), piece("cc", Some(' '))],
        ] {
            let before = texts(&pieces);
            space_transform_pieces(&mut pieces);
            assert_eq!(texts(&pieces), before);
        }
    }
}
