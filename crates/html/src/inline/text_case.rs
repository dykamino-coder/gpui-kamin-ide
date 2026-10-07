//! Unicode letter casing with Dutch IJ titlecase tailoring.
//! CSS Text 3 #text-transform-mapping applies tailoring before glyph shaping.

use crate::computed::{Computed, TextTransform};

/// Титульный регистр знака — там, где он ОТЛИЧАЕТСЯ от прописного.
///
/// Таких мест в Юникоде немного: составные буквы, у которых прописной вариант
/// пишется двумя большими (`ǄǅǆЛЈ…`), и греческие с приданной йотой, где
/// полное прописное отображение даёт ДВА знака. `None` — отличий нет, годится
/// обычное `to_uppercase`.
fn titlecase(ch: char) -> Option<char> {
    let c = ch as u32;
    let title = match c {
        0x01C4..=0x01C6 => 0x01C5,
        0x01C7..=0x01C9 => 0x01C8,
        0x01CA..=0x01CC => 0x01CB,
        0x01F1..=0x01F3 => 0x01F2,
        // Приданная йота: заглавная форма стоит ровно на восемь позиций выше.
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => c + 8,
        0x1FB3 => 0x1FBC,
        0x1FC3 => 0x1FCC,
        0x1FF3 => 0x1FFC,
        _ => return None,
    };
    char::from_u32(title)
}

pub(super) fn apply(text: &str, style: &Computed) -> String {
    match style.text_transform {
        Some(TextTransform::Upper) => text.to_uppercase(),
        // Полноширинные двойники лежат ровно на 0xFEE0 выше своих знаков
        // ASCII; пробел заменяется отдельным знаком.
        Some(TextTransform::FullWidth) => text
            .chars()
            .map(|ch| match ch as u32 {
                0x20 => '\u{3000}',
                c @ 0x21..=0x7E => char::from_u32(c + 0xFEE0).unwrap_or(ch),
                _ => ch,
            })
            .collect(),
        Some(TextTransform::Lower) => text.to_lowercase(),
        Some(TextTransform::Capitalize) => {
            // Начало слова — первая БУКВА (css-text-3 §2.1: «first typographic
            // letter unit of each word»): открывающая скобка и прочая
            // пунктуация перед ней пропускаются (`(é` → `(É`). Границы слов —
            // по UAX #29: `.`, `'`, `:` между буквами слово НЕ рвут (WB6/WB7,
            // `x.x.` → `X.x.`), прочая пунктуация рвёт (`foo-bar` → `Foo-Bar`).
            // Прежде началом считался только знак после пробела.
            // CSS Text 3 #text-transform-mapping: Dutch IJ is one titlecase unit.
            let dutch = style.lang.as_deref().is_some_and(|language| {
                language
                    .split('-')
                    .next()
                    .is_some_and(|primary| primary.eq_ignore_ascii_case("nl"))
            });
            let mut title_j = false;
            let mut out = String::with_capacity(text.len());
            let mut prev: Option<char> = None;
            let mut prev2: Option<char> = None;
            let mid = |c: char| matches!(c, '.' | '\'' | '\u{2019}' | ':' | '\u{b7}');
            for ch in text.chars() {
                let at_start = ch.is_alphabetic()
                    && match prev {
                        None => true,
                        Some(p) if p.is_alphanumeric() => false,
                        Some(p) if mid(p) => !prev2.is_some_and(char::is_alphanumeric),
                        Some(_) => true,
                    };
                if at_start {
                    // ТИТУЛЬНЫЙ регистр, а не прописной (css-text-3 §2.1).
                    // У диграфов и у греческого с приданной йотой это разные
                    // знаки: `ǆ` даёт `ǅ`, а не `Ǆ`; `ᾀ` даёт `ᾈ`, а не пару
                    // `ἈΙ` (`text-transform-capitalize-007` и `-016`).
                    match titlecase(ch) {
                        Some(title) => out.push(title),
                        None => out.extend(ch.to_uppercase()),
                    }
                } else if title_j && ch.eq_ignore_ascii_case(&'j') {
                    out.push('J');
                } else {
                    out.push(ch);
                }
                title_j = dutch && at_start && ch.eq_ignore_ascii_case(&'i');
                prev2 = prev;
                prev = Some(ch);
            }
            out
        }
        _ => text.to_string(),
    }
}
