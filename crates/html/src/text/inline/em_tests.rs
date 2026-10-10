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
