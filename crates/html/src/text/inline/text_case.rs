//! Unicode letter casing with language tailoring (`lang_case`: Turkic, Lithuanian,
//! Greek, Dutch IJ titlecase).
//! CSS Text 3 #text-transform-mapping applies tailoring before glyph shaping.

use super::lang_case;
use crate::computed::{Computed, TextTransform};

/// CSS Text 3 §2.1: inline boundaries do not delimit words, even when
/// the adjoining text has a different text-transform value.
#[derive(Default)]
pub(crate) struct Context {
    prev: Option<char>,
    prev2: Option<char>,
}

impl Context {
    pub(super) fn boundary(&mut self) {
        self.prev = None;
        self.prev2 = None;
    }

    fn observe(&mut self, text: &str) {
        for ch in text.chars() {
            self.prev2 = self.prev;
            self.prev = Some(ch);
        }
    }
}

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

fn apply(text: &str, style: &Computed, context: &mut Context) -> String {
    let (previous, previous2) = (context.prev, context.prev2);
    context.observe(text);
    // Языковые поправки регистра (css-text-3 §2.1.1, SpecialCasing.txt):
    // `lang="tr"` даёт `i` → `İ`, `lang="el"` снимает ударения и т. д.
    let tailoring = lang_case::tailoring(style.lang.as_deref());
    match style.text_transform {
        Some(TextTransform::Upper) => match tailoring {
            Some(t) => lang_case::upper(text, t),
            None => text.to_uppercase(),
        },
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
        Some(TextTransform::Lower) => match tailoring {
            Some(t) => lang_case::lower(text, t),
            None => text.to_lowercase(),
        },
        Some(TextTransform::Capitalize) => {
            // Начало слова — первая БУКВА (css-text-3 §2.1: «first typographic
            // letter unit of each word»): открывающая скобка и прочая
            // пунктуация перед ней пропускаются (`(é` → `(É`). Границы слов —
            // по UAX #29: `.`, `'`, `:` между буквами слово НЕ рвут (WB6/WB7,
            // `x.x.` → `X.x.`), прочая пунктуация рвёт (`foo-bar` → `Foo-Bar`).
            // Прежде началом считался только знак после пробела.
            let mut out = String::with_capacity(text.len());
            let mut prev = previous;
            let mut prev2 = previous2;
            let mid = |c: char| matches!(c, '.' | '\'' | '\u{2019}' | ':' | '\u{b7}');
            let chars: Vec<char> = text.chars().collect();
            let mut skip = 0usize;
            for (i, &ch) in chars.iter().enumerate() {
                if skip > 0 {
                    skip -= 1;
                    prev2 = prev;
                    prev = Some(ch);
                    continue;
                }
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
                    if let Some(taken) = tailoring
                        .and_then(|t| lang_case::title_start(ch, &chars[i + 1..], t, &mut out))
                    {
                        skip = taken;
                    } else {
                        match titlecase(ch) {
                            Some(title) => out.push(title),
                            None => out.extend(ch.to_uppercase()),
                        }
                    }
                } else {
                    out.push(ch);
                }
                prev2 = prev;
                prev = Some(ch);
            }
            out
        }
        _ => text.to_string(),
    }
}

pub(super) fn transform(text: &str, style: &Computed, context: &mut Context) -> String {
    let flags = style.text_transform_flags;
    let cased = apply(text, style, context);
    if flags == 0 {
        return cased;
    }
    // Порядок css-text-3 §2.1: регистр, затем `full-width`, затем
    // `full-size-kana`. `math-auto` — только у текста из ОДНОГО знака
    // (MathML Core §2.1.5 «If the text consists of a single character»).
    let single = {
        let mut it = cased.trim().chars();
        it.next().is_some() && it.next().is_none()
    };
    cased
        .chars()
        .map(|ch| {
            let mut ch = ch;
            if flags & crate::computed::TT_FULL_WIDTH != 0 {
                ch = super::full_width(ch);
            }
            if flags & crate::computed::TT_KANA != 0 {
                ch = super::full_size_kana(ch);
            }
            if flags & crate::computed::TT_MATH != 0 && single {
                ch = super::math_italic(ch);
            }
            ch
        })
        .collect()
}
