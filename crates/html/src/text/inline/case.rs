//! Преобразования текста: text-transform, полноширинные формы, кана, математический курсив.

use crate::style::computed::Computed;
use crate::text::inline::*;

/// Пробелы куска набираются U+3000 (`text-transform: full-width`).
fn full_width_spaces(style: &Computed) -> bool {
    style.text_transform_flags & crate::style::computed::TT_FULL_WIDTH != 0
}

/// `word-space-transform` по КУСКАМ абзаца.
///
/// Соседи точки переноса сплошь и рядом лежат в других кусках: `あ<wbr>い` —
/// это три куска, а `<span>` с полем или рамкой делит абзац и вовсе на
/// отдельные элементы. Замена знак-в-знак: нулевой пробел и идеографический
/// занимают в UTF-8 одинаково, поэтому прогоны не съезжают.
pub fn space_transform_pieces(pieces: &mut [Piece]) {
    let mut seq: Vec<(usize, usize, char)> = vec![];
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
        let Some(sep) = style.word_space_char.filter(|&c| c != '\0') else {
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
            && matches!(&pieces[i], Piece::Text { style, .. } if style.keep_spaces != Some(true))
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

/// `text-transform`: регистр меняется до шейпинга — шрифт про него не знает.
pub fn transform_case(text: &str, style: &Computed) -> String {
    text_case::transform(text, style, &mut text_case::Context::default())
}

/// Полноширинный двойник знака (css-text-3 §2.1 `full-width`: знаки,
/// у которых есть «fullwidth» форма по UAX #11, и полуширинные формы,
/// раскрытые обратно — как ICU `Halfwidth-Fullwidth`).
pub(super) fn full_width(ch: char) -> char {
    let c = ch as u32;
    let m = match c {
        0x20 => 0x3000,
        0x21..=0x7E => c + 0xFEE0,
        0x2985 => 0xFF5F,
        0x2986 => 0xFF60,
        0xA2 => 0xFFE0,
        0xA3 => 0xFFE1,
        0xAC => 0xFFE2,
        0xAF => 0xFFE3,
        0xA6 => 0xFFE4,
        0xA5 => 0xFFE5,
        0x20A9 => 0xFFE6,
        0xFF61..=0xFF9F => HALF_KATAKANA[(c - 0xFF61) as usize] as u32,
        0xFFA0 => 0x3164,
        0xFFA1..=0xFFBE => c - 0xFFA1 + 0x3131,
        0xFFC2..=0xFFC7 => c - 0xFFC2 + 0x314F,
        0xFFCA..=0xFFCF => c - 0xFFCA + 0x3155,
        0xFFD2..=0xFFD7 => c - 0xFFD2 + 0x315B,
        0xFFDA..=0xFFDC => c - 0xFFDA + 0x3161,
        0xFFE8 => 0x2502,
        0xFFE9..=0xFFEC => c - 0xFFE9 + 0x2190,
        0xFFED => 0x25A0,
        0xFFEE => 0x25CB,
        _ => c,
    };
    char::from_u32(m).unwrap_or(ch)
}

/// Полуширинная катакана U+FF61..U+FF9F → полноширинная (UnicodeData,
/// разложение `<narrow>`).
const HALF_KATAKANA: [u16; 63] = [
    0x3002, 0x300C, 0x300D, 0x3001, 0x30FB, 0x30F2, 0x30A1, 0x30A3, 0x30A5, 0x30A7, 0x30A9, 0x30E3,
    0x30E5, 0x30E7, 0x30C3, 0x30FC, 0x30A2, 0x30A4, 0x30A6, 0x30A8, 0x30AA, 0x30AB, 0x30AD, 0x30AF,
    0x30B1, 0x30B3, 0x30B5, 0x30B7, 0x30B9, 0x30BB, 0x30BD, 0x30BF, 0x30C1, 0x30C4, 0x30C6, 0x30C8,
    0x30CA, 0x30CB, 0x30CC, 0x30CD, 0x30CE, 0x30CF, 0x30D2, 0x30D5, 0x30D8, 0x30DB, 0x30DE, 0x30DF,
    0x30E0, 0x30E1, 0x30E2, 0x30E4, 0x30E6, 0x30E8, 0x30E9, 0x30EA, 0x30EB, 0x30EC, 0x30ED, 0x30EF,
    0x30F3, 0x3099, 0x309A,
];

/// Малая кана → полноразмерная (css-text-3 §2.1 `full-size-kana`, таблица
/// «Full-Size Kana Mappings» приложения G).
pub(super) fn full_size_kana(ch: char) -> char {
    match ch {
        'ぁ' => 'あ',
        'ぃ' => 'い',
        'ぅ' => 'う',
        'ぇ' => 'え',
        'ぉ' => 'お',
        'ゕ' => 'か',
        'ゖ' => 'け',
        'っ' => 'つ',
        'ゃ' => 'や',
        'ゅ' => 'ゆ',
        'ょ' => 'よ',
        'ゎ' => 'わ',
        'ァ' => 'ア',
        'ィ' => 'イ',
        'ゥ' => 'ウ',
        'ェ' => 'エ',
        'ォ' => 'オ',
        'ヵ' => 'カ',
        'ㇰ' => 'ク',
        'ヶ' => 'ケ',
        'ㇱ' => 'シ',
        'ㇲ' => 'ス',
        'ッ' => 'ツ',
        'ㇳ' => 'ト',
        'ㇴ' => 'ヌ',
        'ㇵ' => 'ハ',
        'ㇶ' => 'ヒ',
        'ㇷ' => 'フ',
        'ㇸ' => 'ヘ',
        'ㇹ' => 'ホ',
        'ㇺ' => 'ム',
        'ャ' => 'ヤ',
        'ュ' => 'ユ',
        'ョ' => 'ヨ',
        'ㇻ' => 'ラ',
        'ㇼ' => 'リ',
        'ㇽ' => 'ル',
        'ㇾ' => 'レ',
        'ㇿ' => 'ロ',
        'ヮ' => 'ワ',
        'ｧ' => 'ｱ',
        'ｨ' => 'ｲ',
        'ｩ' => 'ｳ',
        'ｪ' => 'ｴ',
        'ｫ' => 'ｵ',
        'ｯ' => 'ﾂ',
        'ｬ' => 'ﾔ',
        'ｭ' => 'ﾕ',
        'ｮ' => 'ﾖ',
        _ => ch,
    }
}

/// Курсивный математический двойник (MathML Core §2.1.5, «italic mappings»).
pub(super) fn math_italic(ch: char) -> char {
    let c = ch as u32;
    let m = match c {
        0x68 => 0x210E,
        0x41..=0x5A => 0x1D434 + (c - 0x41),
        0x61..=0x7A => 0x1D44E + (c - 0x61),
        0x131 => 0x1D6A4,
        0x237 => 0x1D6A5,
        0x391..=0x3A1 => 0x1D6E2 + (c - 0x391),
        0x3F4 => 0x1D6F3,
        0x3A3..=0x3A9 => 0x1D6F4 + (c - 0x3A3),
        0x2207 => 0x1D6FB,
        0x3B1..=0x3C9 => 0x1D6FC + (c - 0x3B1),
        0x2202 => 0x1D715,
        0x3F5 => 0x1D716,
        0x3D1 => 0x1D717,
        0x3F0 => 0x1D718,
        0x3D5 => 0x1D719,
        0x3F1 => 0x1D71A,
        0x3D6 => 0x1D71B,
        _ => c,
    };
    char::from_u32(m).unwrap_or(ch)
}
