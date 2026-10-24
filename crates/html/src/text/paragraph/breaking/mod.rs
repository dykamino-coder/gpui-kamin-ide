//! Разбиение на строки: split, lay_in, балансировка, аварийные разрывы, места переноса (UAX #14).

mod cluster_edges;
mod punctuation;
pub(super) use crate::text::paragraph::breaking::cluster_edges::cluster_edge;
use crate::text::paragraph::breaking::cluster_edges::cluster_edge_at;
use crate::text::paragraph::breaking::punctuation::centered_punctuation;
use crate::text::paragraph::breaking::punctuation::conditional_japanese_starter;
pub(super) use crate::text::paragraph::breaking::punctuation::is_closing;
pub(super) use crate::text::paragraph::breaking::punctuation::is_opening;
pub(super) use crate::text::paragraph::breaking::punctuation::is_stop;
use crate::text::paragraph::breaking::punctuation::iteration_mark;
use crate::text::paragraph::breaking::punctuation::letter_unit;
use crate::text::paragraph::breaking::punctuation::no_break_after;
use crate::text::paragraph::breaking::punctuation::no_break_before;
use crate::text::paragraph::breaking::punctuation::wide_postfix;
use crate::text::paragraph::breaking::punctuation::wide_prefix;

mod cut;
mod layout_lines;
mod opportunities;
mod soft_opportunities;
mod split_lines;
mod unicode_breaks;
mod wrap_policy;

use crate::text::paragraph::*;

/// Правила переноса из стиля БЕЗ вопроса, нужна ли своя раскладка.
pub fn wrap_of(c: &crate::style::computed::Computed) -> Wrap {
    Wrap {
        nowrap: c.nowrap == Some(true),
        break_spaces: crate::dom::whitespace::break_spaces(c),
        break_all: c.break_anywhere == Some(true) && c.break_anywhere_strict != Some(true),
        anywhere: c.break_anywhere_strict == Some(true),
        keep_all: c.keep_all == Some(true),
        auto_phrase: c.auto_phrase == Some(true),
        // `word-break: break-word` — устаревшая запись `normal` с
        // `overflow-wrap: anywhere` (css-text-4 §5.2).
        break_word: c.break_word == Some(true) || c.word_break_word == Some(true),
        wrap_anywhere: c.wrap_anywhere == Some(true) || c.word_break_word == Some(true),
        rtl: c.wm().rtl(),
        balance: c.balance_lines == Some(true),
        keep_spaces: crate::dom::whitespace::keep_spaces(c),
        loose: c.line_break_loose.unwrap_or(0),
        cjk_lang: c.lang.as_deref().is_some_and(cjk_writing),
    }
}

/// Chinese or Japanese content for `line-break` (css-text-3 §5.3): the
/// language itself, or a Chinese/Japanese script subtag (BCP 47) even under
/// another language (`writing-system-line-break-002`: `en-Hrkt`).
fn cjk_writing(lang: &str) -> bool {
    let mut subtags = lang.split(['-', '_']);
    let language = subtags.next().unwrap_or_default();
    if let Some(script) =
        subtags.find(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()))
    {
        return ["hrkt", "hira", "kana", "jpan", "hani", "hans", "hant"]
            .iter()
            .any(|s| script.eq_ignore_ascii_case(s));
    }
    language.eq_ignore_ascii_case("ja") || language.eq_ignore_ascii_case("zh")
}

#[cfg(test)]
mod tests {
    //! Regression tests for CSS class tailoring before Unicode line breaking.
    use crate::text::paragraph::*;
    use gpui::{SharedString, px};

    fn paragraph(text: &str) -> Paragraph {
        Paragraph::new(
            SharedString::from(text.to_owned()),
            vec![],
            px(16.),
            px(16.),
            Align::Left,
            Wrap {
                break_all: true,
                ..Default::default()
            },
        )
    }

    fn stops(p: &Paragraph) -> Vec<usize> {
        p.opportunities().into_iter().map(|s| s.at).collect()
    }

    #[test]
    fn break_all_preserves_after_punctuation_and_source_byte_offsets() {
        assert_eq!(stops(&paragraph("በ፡በ፡በ")), vec![6, 12]);
        assert_eq!(stops(&paragraph("é1β")), vec![2, 3]);
    }

    #[test]
    fn break_all_preserves_glue_and_combining_clusters() {
        assert!(stops(&paragraph("a\u{2060}b")).is_empty());
        assert_eq!(stops(&paragraph("a\u{301}b")), vec![3]);
        assert!(stops(&paragraph("a\u{a0}b")).is_empty());
    }

    #[test]
    fn break_all_tailoring_is_scoped_and_does_not_enable_emergency_breaks() {
        let mut p = paragraph("aabbcc");
        p.wrap = Wrap::default();
        p.spans = vec![(
            2..4,
            Wrap {
                break_all: true,
                ..Default::default()
            },
        )];
        assert_eq!(stops(&p), vec![2, 3, 4]);
        assert!(!p.emergency_ok(2));
        assert!(!p.emergency_inside(0, 6));
    }

    #[test]
    fn loose_hyphens_use_the_tailored_preceding_class() {
        for text in ["aa‐a", "aa–a"] {
            let mut p = paragraph(text);
            p.wrap.loose = 2;
            assert!(stops(&p).contains(&2), "{text}");
        }
    }

    #[test]
    fn explicit_writing_system_precedes_the_language_default() {
        for lang in ["ja", "ZH", "ja-Jpan", "en-Hrkt", "ko-Hani"] {
            assert!(super::cjk_writing(lang), "{lang}");
        }
        for lang in ["ja-Hang", "zh-Latn", "ko", "en", "jargon"] {
            assert!(!super::cjk_writing(lang), "{lang}");
        }
    }

    #[test]
    fn auto_phrase_retains_hyphenation_for_overflow_only() {
        let mut p = paragraph("ab\u{ad}cd");
        p.wrap.break_all = false;
        p.wrap.auto_phrase = true;
        assert!(stops(&p).is_empty());
        assert!(
            p.opportunities_with_fallback(true)
                .iter()
                .any(|s| s.at == 4)
        );
    }
}
