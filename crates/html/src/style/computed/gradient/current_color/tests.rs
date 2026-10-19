//! Inherited gradient operands, current colors and computed font positions.

use super::*;
use crate::style::computed::GradSpace;

#[test]
fn shorthand_inheritance_keeps_the_native_background_route() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one(
        "background",
        "linear-gradient(to right, currentcolor, limegreen)",
        vcx,
    );
    let mut child = Computed::default();
    child.apply_one("background", "inherit", vcx);
    child.apply_one("color", "limegreen", vcx);
    let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
    assert!(child.gradient_raw.is_none());
    let gradient = child.gradient.as_ref().unwrap();
    assert_eq!(gradient.from, Color::parse("limegreen", vcx).unwrap());
    assert_eq!(gradient.to, gradient.from);
    // Canvas propagation moves the painted gradient away from body,
    // while descendants still inherit its authored image expression.
    parent.gradient = None;
    let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
    assert_eq!(
        child.gradient.unwrap().from,
        Color::parse("limegreen", vcx).unwrap()
    );
}

#[test]
fn relative_stops_use_the_inherited_element_color() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one(
        "background-image",
        "linear-gradient(rgb(from currentcolor r g b), rgb(from currentcolor r g b))",
        vcx,
    );
    let mut parent = Computed::default();
    parent.apply_one("color", "blue", vcx);
    let blue = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
    let gradient = blue.gradient.as_ref().unwrap();
    assert_eq!(gradient.from, Color::parse("blue", vcx).unwrap());
    assert!(matches!(gradient.space, GradSpace::Oklab));
    let mut child = Computed::default();
    child.apply_one("color", "green", vcx);
    child.apply_one("background-image", "inherit", vcx);
    let green = crate::style::cascade::inherit::inherit(&blue, &child, vcx);
    assert_eq!(
        green.gradient.unwrap().from,
        Color::parse("green", vcx).unwrap()
    );
    assert_eq!(
        green.gradient_raw.unwrap().gradient().unwrap().0.from,
        Color::parse("green", vcx).unwrap()
    );
}

#[test]
fn cancelled_current_color_image_is_not_reinherited() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for replacement in ["none", "image(red)"] {
        let mut parent = Computed::default();
        parent.apply_one(
            "background-image",
            "linear-gradient(currentcolor, currentcolor)",
            vcx,
        );
        parent.apply_one("background-image", replacement, vcx);
        let mut child = Computed::default();
        child.apply_one("background-image", "inherit", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
        assert!(parent.gradient_current.is_none());
        assert!(child.gradient.is_none());
    }
}
#[test]
fn typed_stop_colors_preserve_double_positions_without_css() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one(
        "background-image",
        "linear-gradient(in srgb, currentcolor 20px 30px, blue 100px)",
        vcx,
    );
    // The inherited operand must not consult a textual paint source.
    parent.gradient_raw = Some(crate::style::computed::ImageSource::Invalid);
    let mut child = Computed::default();
    child.apply_one("background-image", "inherit", vcx);
    child.apply_one("color", "red", vcx);
    let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
    let g = child.gradient.unwrap();
    let red = Color::parse("red", vcx).unwrap();
    assert_eq!(
        g.stops_px,
        vec![
            (red, 20.0),
            (red, 30.0),
            (Color::parse("blue", vcx).unwrap(), 100.0)
        ]
    );
    assert_eq!(
        g.stops_raw.iter().map(|s| s.2).collect::<Vec<_>>(),
        vec![Some(20.0), Some(30.0), Some(100.0)]
    );
}

#[test]
fn inherited_template_preserves_computed_font_positions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one(
        "background-image",
        "linear-gradient(currentcolor 1em, blue 2em)",
        vcx,
    );
    let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
    let mut child = Computed::default();
    child.apply_one("font-size", "80px", vcx);
    child.apply_one("background-image", "inherit", vcx);
    child.apply_one("color", "green", vcx);
    let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
    let g = child.gradient.unwrap();
    assert_eq!(g.stops_px[0], (Color::parse("green", vcx).unwrap(), 20.0));
    assert_eq!(g.stops_px[1].1, 40.0);
    assert!(g.stop_lengths.is_empty());
}
#[test]
fn radial_first_stop_accepts_current_and_relative_colors() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let red = Color::parse("red", vcx).unwrap();
    for token in ["currentcolor", "rgb(from currentcolor r g b)"] {
        let g = parse_gradient_with_current(&format!("radial-gradient({token}, blue)"), red, vcx)
            .unwrap();
        assert_eq!(g.stops.len(), 2);
        assert_eq!(g.from, red);
        assert_eq!(g.to, Color::parse("blue", vcx).unwrap());
    }
}
