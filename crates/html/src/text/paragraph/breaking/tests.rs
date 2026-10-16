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
