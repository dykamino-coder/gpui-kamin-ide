//! Select first-letter text across inline text fragments before styling it.

use super::{Piece, SPACER, ZWSP, bidi_format};
use crate::style::computed::Computed;

/// CSS 2.1 §5.12.2 and CSS Pseudo §first-letter-tree permit a first letter
/// spanning multiple elements, including generated punctuation. Select the
/// complete typographic unit before mapping it back to individual runs.
pub fn split_first_letter(pieces: Vec<Piece>, style: &Computed) -> Vec<Piece> {
    let mut text = String::new();
    let mut ranges = Vec::with_capacity(pieces.len());
    let mut blocked = false;
    for piece in &pieces {
        let range = match piece {
            Piece::Atom(_) => {
                blocked = true;
                None
            }
            Piece::Text {
                text: part,
                style: own,
            } if !own.first_letter_excluded
                && !blocked
                && part != SPACER
                && part != ZWSP
                && !part.chars().all(bidi_format) =>
            {
                let start = text.len();
                // CSS 2.2 section 5.12.2: a letter after a hard break is
                // not on the first formatted line, even when that line is empty.
                let end = part.find('\n').unwrap_or(part.len());
                text.push_str(&part[..end]);
                blocked = end < part.len();
                Some(start..text.len())
            }
            _ => None,
        };
        ranges.push(range);
    }
    let Some((pos, _)) = text.char_indices().find(|(_, c)| !c.is_whitespace()) else {
        return pieces;
    };
    let start = ranges
        .iter()
        .flatten()
        .find(|r| r.contains(&pos))
        .unwrap()
        .start;
    let end = first_letter_end(&text, pos);
    let mut out = Vec::with_capacity(pieces.len() + 2);
    for (piece, range) in pieces.into_iter().zip(ranges) {
        match (piece, range) {
            (Piece::Text { text, style: own }, Some(r)) if r.start < end && r.end > start => {
                let a = start.saturating_sub(r.start);
                let b = end.min(r.end) - r.start;
                if a > 0 {
                    out.push(Piece::Text {
                        text: text[..a].into(),
                        style: own.clone(),
                    });
                }
                out.push(Piece::Text {
                    text: text[a..b].into(),
                    style: letter_style(&own, style),
                });
                if b < text.len() {
                    out.push(Piece::Text {
                        text: text[b..].into(),
                        style: own,
                    });
                }
            }
            (other, _) => out.push(other),
        }
    }
    out
}

fn letter_style(own: &Computed, style: &Computed) -> Computed {
    let mut letter = own.clone();
    letter.font_size = style.font_size.or(own.font_size);
    letter.line_height = style.line_height.or(own.line_height);
    letter.color = style.color.or(own.color);
    letter.font_weight = style.font_weight.or(own.font_weight);
    letter.italic = style.italic.or(own.italic);
    letter.font_family = style.font_family.clone().or(own.font_family.clone());
    if let Some(bg) = style.background {
        letter.inline_bg = Some(bg);
    }
    letter
}

/// Unicode categories, rather than script ranges, define first-letter punctuation.
/// CSS 2.1 §5.12.2 includes Ps/Pe/Pi/Pf/Po; CSS Pseudo §first-letter
/// additionally includes Pc/Pd in the leading punctuation sequence.
fn is_punct(c: char) -> bool {
    use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
    c.general_category_group() == GeneralCategoryGroup::Punctuation
}

/// CSS Pseudo §first-letter excludes Ps/Pd from trailing punctuation.
fn open_or_dash(c: char) -> bool {
    use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
    matches!(
        c.general_category(),
        GeneralCategory::OpenPunctuation | GeneralCategory::DashPunctuation
    )
}

/// Типографский пробел `Zs` без U+3000 (оба правила спеки его исключают).
fn typographic_space(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}'
    )
}

/// Смещение знака в блоках брахми (деванагари…малаялам повторяют одну
/// раскладку ISCII): вирама — 0x4D, согласные — 0x15…0x39.
fn brahmic_offset(c: char) -> Option<u32> {
    let u = c as u32;
    (0x0900..=0x0D7F).contains(&u).then_some(u & 0x7F)
}

/// Знак, который держится за предыдущую букву: комбинирующие диакритики,
/// селекторы начертания, ZWJ и зависимые знаки брахми (огласовки, вирама,
/// анусвара).
fn joins_letter(c: char) -> bool {
    matches!(
        c as u32,
        0x300..=0x36F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F | 0x200D
    ) || brahmic_offset(c).is_some_and(
        |o| matches!(o, 0x00..=0x03 | 0x3A..=0x3C | 0x3E..=0x4F | 0x51..=0x57 | 0x62..=0x63),
    )
}

/// Конец текста первой буквы (css-pseudo-4 §first-letter): ведущая
/// пунктуация с пробелами между нею и буквой, сама буква — «typographic
/// letter unit» с диакритиками и слитными согласными брахми (вирама +
/// согласная, `first-letter-hi-001`: «स्थ», а не «स»), затем хвост из
/// пунктуации кроме `Ps`/`Pd` с пробелами-не-разделителями слов между.
/// Шаблон спеки: `(P (Zs|P)*)? (L|N|S) ((Zs|P−(Ps|Pd))* (P−(Ps|Pd)))?`.
/// Нет буквы за ведущей пунктуацией — прежний разрез по первому знаку.
fn first_letter_end(text: &str, start: usize) -> usize {
    let chars: Vec<(usize, char)> = text[start..]
        .char_indices()
        .map(|(i, c)| (start + i, c))
        .collect();
    let end_at = |k: usize| chars.get(k).map_or(text.len(), |(i, _)| *i);
    let mut k = 0;
    if chars.first().is_some_and(|(_, c)| is_punct(*c)) {
        while chars
            .get(k)
            .is_some_and(|(_, c)| is_punct(*c) || typographic_space(*c))
        {
            k += 1;
        }
    }
    if !chars
        .get(k)
        .is_some_and(|(_, c)| !c.is_whitespace() && !is_punct(*c))
    {
        return end_at(1);
    }
    k += 1;
    while let Some(&(_, c)) = chars.get(k) {
        let after_virama = brahmic_offset(chars[k - 1].1) == Some(0x4D);
        let consonant = brahmic_offset(c).is_some_and(|o| (0x15..=0x39).contains(&o));
        if joins_letter(c) || (after_virama && consonant) {
            k += 1;
        } else {
            break;
        }
    }
    // Хвост: пробелы (кроме разделителей слов U+0020/U+00A0) допустимы
    // только МЕЖДУ знаками пунктуации — висящий пробел в букву не входит
    // («T.&emsp;est» → «T.», а «T&emsp;.est» → «T&emsp;.»).
    let (mut end, mut j) = (k, k);
    while let Some(&(_, c)) = chars.get(j) {
        if is_punct(c) && !open_or_dash(c) {
            j += 1;
            end = j;
        } else if typographic_space(c) && !matches!(c, ' ' | '\u{A0}') {
            j += 1;
        } else {
            break;
        }
    }
    end_at(end)
}
