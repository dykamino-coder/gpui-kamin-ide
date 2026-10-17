//! Source offsets must survive generated bidi isolation around inline content.

use super::*;

#[test]
fn generated_controls_do_not_shorten_the_styled_source_range() {
    let piece = |text: &str, inline_control| Piece::Text {
        text: text.into(),
        style: Computed {
            inline_control,
            ..Computed::default()
        },
    };
    let first = Computed {
        font_weight: Some(700),
        ..Computed::default()
    };
    for (generated, at, expected) in [(true, 2, "ab"), (false, 5, "ab")] {
        let pieces = style_first_line(
            vec![piece("\u{2068}", generated), piece("abc", false)],
            at,
            &first,
            &Computed::default(),
        );
        let styled: String = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Text { text, style }
                    if !text.contains('\u{2068}') && style.font_weight == Some(700) =>
                {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(styled, expected);
    }
}
