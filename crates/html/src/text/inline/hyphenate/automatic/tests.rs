//! Dictionary and conditional-hyphenation regression tests.

use super::*;

fn style(lang: &str) -> Computed {
    Computed {
        lang: Some(lang.into()),
        ..Computed::default()
    }
}

#[test]
fn dictionary_points_and_language_tags() {
    for (lang, word, expected) in [
        ("en-US", "example", vec![2, 4]),
        ("nl", "cafeetje", vec![2]),
        ("hu", "ablakok", vec![2, 4]),
    ] {
        assert_eq!(points(word, &style(lang)), expected, "{lang}: {word}");
    }
    assert!(points("example", &style("english")).is_empty());
    assert!(points("exam\u{00ad}ple", &style("en")).is_empty());
}

#[test]
fn character_limits_filter_dictionary_points() {
    let mut style = style("en");
    style.hyphen_limit_chars = Some([None, Some(3), Some(2)]);
    assert_eq!(points("example", &style), [4]);
    style.hyphen_limit_chars = Some([Some(8), None, None]);
    assert!(points("example", &style).is_empty());
}

#[test]
fn manual_priority_is_per_word_and_words_span_markup() {
    let make = |text: &str| Piece::Text {
        text: text.into(),
        style: Computed {
            hyphens_auto: Some(true),
            ..style("en")
        },
    };
    let mut pieces = [make("exam\u{ad}ple example"), make(" exam"), make("ple")];
    hyphenate_pieces(&mut pieces);
    let text: Vec<&str> = pieces
        .iter()
        .filter_map(|piece| {
            if let Piece::Text { text, .. } = piece {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        text,
        [
            "exam\u{ad}ple ex\u{ad}am\u{ad}ple",
            " ex\u{ad}am",
            "\u{ad}ple"
        ]
    );
}

#[test]
fn minimum_lengths_exclude_nonspacing_marks_and_punctuation() {
    assert_eq!(character_count("tu\u{301}’a\u{300}n"), 4);
    assert_eq!(character_count("का"), 2);
}
