//! Em tests for inline; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

use crate::style::cascade::inherit::inherit;
use crate::style::css::parse_decls;

fn styled(css: &str) -> Computed {
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(css));
    c
}

#[test]
fn em_resolves_against_the_font_size() {
    // Родитель 12px, свой размер 2em = 24px, высота 1em = 24px.
    let parent = styled("font-size: 12px");
    let child = styled("font-size: 2em; height: 1em");
    let merged = inherit(&parent, &child);
    assert_eq!(
        merged.font_size,
        Some(Len::Px(24.0)),
        "свой кегль от родителя"
    );
    assert_eq!(merged.height, Some(Len::Px(24.0)), "высота от своего кегля");

    // Внук: 2em от 24 = 48, высота 1em = 48.
    let grand = styled("font-size: 2em; height: 1em");
    let merged2 = inherit(&merged, &grand);
    assert_eq!(merged2.font_size, Some(Len::Px(48.0)));
    assert_eq!(merged2.height, Some(Len::Px(48.0)));
}

#[test]
fn em_without_own_font_size_uses_the_inherited_one() {
    let parent = styled("font-size: 20px");
    let child = styled("width: 10em");
    let merged = inherit(&parent, &child);
    assert_eq!(merged.width, Some(Len::Px(200.0)));
}

#[test]
fn inherited_emphasis_resolves_the_childs_typographic_context() {
    let parent = styled("text-emphasis-style: open; text-emphasis-position: auto");
    let child = inherit(&parent, &styled("writing-mode: vertical-rl"));
    let pieces = [super::Piece::Text {
        text: "試験".into(),
        style: child,
    }];
    let spans = super::emphasis_spans(&pieces, 16.0, 1.2, Some(false), None);
    assert_eq!(spans[0].mark, "\u{fe46}");
    assert!(!spans[0].under);

    let mut child = inherit(&parent, &Computed::default());
    child.lang = Some("Zh-Hant".into());
    let pieces = [super::Piece::Text {
        text: "你好".into(),
        style: child,
    }];
    let spans = super::emphasis_spans(&pieces, 16.0, 1.2, None, None);
    assert_eq!(spans[0].mark, "\u{25cb}");
    assert!(spans[0].under);
}

#[test]
fn explicit_emphasis_none_and_position_override_the_parent() {
    let parent = styled("text-emphasis: sesame; text-emphasis-position: under left");
    let child = inherit(
        &parent,
        &styled("text-emphasis-style: none; text-emphasis-position: over right"),
    );
    assert_eq!(child.text_emphasis.as_deref(), Some(""));
    assert!(!child.emphasis_under);
    assert!(!child.emphasis_left);
    let pieces = [super::Piece::Text {
        text: "text".into(),
        style: child,
    }];
    assert!(super::emphasis_spans(&pieces, 16.0, 1.2, None, None).is_empty());
}
