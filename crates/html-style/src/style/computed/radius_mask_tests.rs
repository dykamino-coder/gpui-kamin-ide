//! Tests: which elliptical corners a quad draws and which still take the
//! group mask (a box that returns to the mask is a paint group again).

use crate::style::computed::{BoxStyle, Computed};

fn styled(declarations: &[(&str, &str)], vcx: crate::values::ValueCx<'_>) -> Computed {
    let mut own = Computed::default();
    for (property, value) in declarations {
        own.apply_one(property, value, vcx);
    }
    let inherited = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    Computed::clone(&inherited)
}

/// (quad draws the corners, mask needed, refusal) for `declarations`.
fn decide(declarations: &[(&str, &str)]) -> (bool, bool, Option<&'static str>) {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let style = styled(declarations, vcx);
    (
        style.radius_elliptic(vcx),
        style.radius_masked(vcx),
        style.ellipse_quad_refusal(),
    )
}

#[test]
fn elliptical_corners_with_a_solid_border_are_drawn_by_the_quad() {
    let decided = decide(&[
        ("border-radius", "20px / 10px"),
        ("border", "2px solid red"),
    ]);
    assert_eq!(decided, (true, false, None));
}

#[test]
fn elliptical_corners_with_a_double_border_are_drawn_by_the_quad() {
    let decided = decide(&[
        ("border-radius", "20px / 10px"),
        ("border", "6px double red"),
    ]);
    assert_eq!(decided, (true, false, None));
}

#[test]
fn elliptical_corners_with_sides_of_two_colours_are_drawn_by_the_quad() {
    let decided = decide(&[
        ("border-radius", "20px / 10px"),
        ("border", "4px solid red"),
        ("border-top-color", "blue"),
    ]);
    assert_eq!(decided, (true, false, None));
}

#[test]
fn elliptical_corners_with_an_inner_shadow_keep_the_mask() {
    // The inner shadow's ring is cut to the elliptical padding edge only by
    // the mask; drawn by the quad it lay square (and rotated with the box).
    for shadow in [
        "inset -2px 2px 0 0 red",
        "inset 0 0 4px 1px red",
        "inset -1em 1em red",
    ] {
        let decided = decide(&[
            ("border-top-right-radius", "14px 29px"),
            ("box-shadow", shadow),
        ]);
        assert_eq!(decided, (false, true, Some("inset_shadow")), "{shadow}");
    }
}

#[test]
fn elliptical_corners_with_an_outer_shadow_stay_with_the_quad() {
    let decided = decide(&[
        ("border-radius", "50% / 30%"),
        ("width", "200px"),
        ("height", "100px"),
        ("box-shadow", "0 0 4px 2px red"),
    ]);
    assert_eq!(decided, (true, false, None));
}

#[test]
fn elliptical_corners_that_clip_content_keep_the_mask() {
    let decided = decide(&[("border-radius", "20px / 10px"), ("overflow", "hidden")]);
    assert_eq!(decided, (false, true, Some("clip")));
}
