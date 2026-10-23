//! `currentcolor` как вычисленное значение (css-color-4 §6.4): слово
//! наследуется словом и решается цветом того элемента, где используется;
//! функция от него в `color` решается цветом родителя.

use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::{Color, StyleColor};

fn merged(parent: &Computed, decls: &[(&str, &str)]) -> Computed {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    for (key, value) in decls {
        own.apply_one(key, value, vcx);
    }
    (inherit(
        &crate::style::computed::BoxStyle::from(parent.clone()),
        &own,
        vcx,
    ))
    .into_computed()
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

#[test]
fn color_function_of_currentcolor_uses_the_parent_color_and_inherits_absolute() {
    let parent = merged(&Computed::default(), &[("color", "rgb(0, 0, 255)")]);
    let mixed = merged(
        &parent,
        &[(
            "color",
            "color-mix(in srgb, currentColor 50%, rgb(255, 255, 255))",
        )],
    );
    let expected = Color {
        r: 128.0 / 255.0,
        g: 128.0 / 255.0,
        b: 1.0,
        a: 1.0,
    };
    let got = mixed.color.unwrap();
    assert!((got.r - expected.r).abs() < 0.01 && (got.b - 1.0).abs() < 0.01);
    let child = merged(&mixed, &[("color", "inherit")]);
    assert_eq!(child.color, mixed.color);
    // `currentcolor` in `color` is `inherit`.
    let child = merged(&parent, &[("color", "currentcolor")]);
    assert_eq!(child.color, parent.color);
}

#[test]
fn inherited_border_currentcolor_resolves_with_the_child_color() {
    let parent = merged(
        &Computed::default(),
        &[("color", "red"), ("border", "2px solid currentcolor")],
    );
    assert_eq!(parent.used_border_color(0), rgb(1.0, 0.0, 0.0));
    let child = merged(
        &parent,
        &[("color", "rgb(0, 128, 0)"), ("border-top-color", "inherit")],
    );
    assert!(child.border_colors[0].is_current());
    assert_eq!(child.used_border_color(0), rgb(0.0, 128.0 / 255.0, 0.0));
}

#[test]
fn background_inherit_keeps_the_keyword() {
    let parent = merged(
        &Computed::default(),
        &[("color", "red"), ("background", "currentColor")],
    );
    assert_eq!(parent.background, Some(rgb(1.0, 0.0, 0.0)));
    let child = merged(&parent, &[("color", "blue"), ("background", "inherit")]);
    assert_eq!(child.background_color, Some(StyleColor::Current));
    assert_eq!(child.background, Some(rgb(0.0, 0.0, 1.0)));
}

#[test]
fn light_dark_branch_may_be_currentcolor() {
    let c = merged(
        &Computed::default(),
        &[
            ("color", "blue"),
            ("background-color", "light-dark(currentColor, currentColor)"),
        ],
    );
    assert_eq!(c.background, Some(rgb(0.0, 0.0, 1.0)));
}

#[test]
fn declared_currentcolor_overrides_an_inherited_emphasis_color() {
    let grand = merged(&Computed::default(), &[("text-emphasis-color", "red")]);
    let parent = merged(
        &grand,
        &[("color", "blue"), ("text-emphasis-color", "currentcolor")],
    );
    let child = merged(&parent, &[("color", "rgb(0, 128, 0)")]);
    let emphasis = child.emphasis_color.as_ref().unwrap();
    assert_eq!(child.used_color(emphasis), rgb(0.0, 128.0 / 255.0, 0.0));
}

#[test]
fn caret_and_accent_colors_inherit() {
    let parent = merged(
        &Computed::default(),
        &[("accent-color", "currentcolor"), ("caret-color", "red")],
    );
    let child = merged(&parent, &[("color", "blue")]);
    assert_eq!(child.used_accent_color(), Some(rgb(0.0, 0.0, 1.0)));
    assert_eq!(child.used_caret_color(), Some(rgb(1.0, 0.0, 0.0)));
}

#[test]
fn shadow_without_color_is_currentcolor() {
    let c = merged(
        &Computed::default(),
        &[("color", "blue"), ("box-shadow", "1px 1px")],
    );
    assert!(c.shadows[0].color.is_current());
    assert_eq!(
        c.shadows[0].used_color(c.current_color()),
        rgb(0.0, 0.0, 1.0)
    );
}

#[test]
fn nested_color_mix_share_belongs_to_its_own_function() {
    let parent = merged(&Computed::default(), &[("color", "color(srgb 0 0 1)")]);
    let c = merged(
        &parent,
        &[(
            "color",
            "color-mix(in srgb, color-mix(in srgb, currentColor 50%, color(srgb 0 1 0)), white)",
        )],
    );
    let got = c.color.unwrap();
    // inner: (0, .5, .5); outer with white: (.5, .75, .75).
    assert!(
        (got.r - 0.5).abs() < 0.01 && (got.g - 0.75).abs() < 0.01 && (got.b - 0.75).abs() < 0.01
    );
}
