//! Validate currentColor as one deferred color component of a border shorthand.

use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};

#[test]
fn currentcolor_does_not_invalidate_the_border_width_and_style() {
    let mut own = Computed::default();
    own.apply_one("border", "1px solid currentColor");
    own.color = Color::parse("green");
    let painted = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
    assert_eq!(painted.borders().top, Some(Len::Px(1.0)));
    assert_eq!(painted.border_color, own.color);
    assert_eq!(painted.border_side_styles, [Some(9); 4]);
}

#[test]
fn two_color_components_invalidate_the_entire_declaration() {
    let mut c = Computed::default();
    c.apply_one("border", "2px solid green");
    let accepted = c.clone();
    for value in [
        "1px solid red currentcolor",
        "1px solid currentcolor currentcolor",
    ] {
        c.apply_one("border", value);
        assert_eq!(c.border_width, accepted.border_width);
        assert_eq!(c.border_color, accepted.border_color);
    }
}
