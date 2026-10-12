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

#[test]
fn initial_opposite_overflow_axis_computes_to_auto_without_becoming_forced() {
    use crate::style::computed::Overflow;
    let mut own = Computed::default();
    own.apply_one("overflow-x", "scroll");
    let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
    assert_eq!([c.overflow_x, c.overflow_y], [Some(Overflow::Scroll); 2]);
    assert_eq!(c.scroll.forced, [true, false]);
    let child = crate::style::cascade::inherit::inherit(&c, &Computed::default());
    assert_eq!([child.overflow_x, child.overflow_y], [None; 2]);
}

#[test]
fn clip_preserves_non_scroll_pairs_and_computes_to_hidden_beside_auto() {
    use crate::style::computed::Overflow;
    for (declaration, expected) in [
        (
            "visible clip",
            [Some(Overflow::Visible), Some(Overflow::Clip)],
        ),
        (
            "clip auto",
            [Some(Overflow::Hidden), Some(Overflow::Scroll)],
        ),
    ] {
        let mut own = Computed::default();
        own.apply_one("overflow", declaration);
        let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
        assert_eq!([c.overflow_x, c.overflow_y], expected);
        assert_eq!(c.scroll.forced, [false; 2]);
    }
}

#[test]
fn root_used_gutter_suppression_does_not_change_explicit_computed_inheritance() {
    let mut parent = Computed::default();
    parent.apply_one("scrollbar-gutter", "both-edges  stable");
    parent.scroll.viewport_root = true;
    let mut own = Computed::default();
    own.apply_one("scrollbar-gutter", "inherit");
    own.apply_one("scrollbar-gutter", "both-edges"); // invalid declaration retains inherit
    let child = crate::style::cascade::inherit::inherit(&parent, &own);
    assert_eq!(child.scroll.gutter, Some(2));
    assert!(!child.scroll.viewport_root);
    let local = crate::style::cascade::inherit::inherit(&parent, &Computed::default());
    assert_eq!(local.scroll.gutter, None);
}
