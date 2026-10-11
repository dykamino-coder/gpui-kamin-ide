//! Regression checks for the cascade boundary of HTML scrollbar policies.
use crate::style::computed::Computed;

#[test]
fn scrollbar_color_inherits_but_geometry_does_not() {
    let mut parent = Computed::default();
    parent.apply_one("scrollbar-color", "rgb(255, 0, 0) blue");
    parent.apply_one("scrollbar-width", "none");
    let child = crate::style::cascade::inherit::inherit(&parent, &Computed::default());
    assert_eq!(child.scroll.colors, parent.scroll.colors);
    assert_eq!(child.scroll.width, None);
    let mut reset = Computed::default();
    reset.apply_one("scrollbar-color", "initial");
    let child = crate::style::cascade::inherit::inherit(&parent, &reset);
    assert_eq!(child.scroll.colors, Some(None));
}

#[test]
fn inherited_scrollbar_current_color_keeps_the_parent_computed_color() {
    let mut declaration = Computed::default();
    declaration.apply_one("color", "red");
    declaration.apply_one("scrollbar-color", "currentColor blue");
    let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &declaration);
    let mut own = Computed::default();
    own.apply_one("color", "green");
    let child = crate::style::cascade::inherit::inherit(&parent, &own);
    assert_eq!(child.scroll.colors, parent.scroll.colors);
    own.apply_one("scrollbar-color", "currentColor blue");
    let child = crate::style::cascade::inherit::inherit(&parent, &own);
    assert_eq!(
        child.scroll.colors.flatten().unwrap()[0],
        child.color.unwrap()
    );
    assert_ne!(child.scroll.colors, parent.scroll.colors);
}

#[test]
fn invalid_scrollbar_color_preserves_the_last_valid_declaration() {
    let mut c = Computed::default();
    c.apply_one("scrollbar-color", "red blue");
    let valid = c.scroll.colors;
    for invalid in ["red", "red blue green", "red not-a-color"] {
        c.apply_one("scrollbar-color", invalid);
        assert_eq!(c.scroll.colors, valid);
    }
}
