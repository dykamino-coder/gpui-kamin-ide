//! Переход между пользовательскими единицами SVG и CSS-пикселями HTML.

use super::*;

#[test]
fn lengths_preserve_svg_units_for_css_attributes_and_percentages() {
    let parsed = crate::dom::parse("", "");
    let vcx = parsed.values();
    assert_eq!(
        length(Some(Len::Px(200.0)), Some("60"), 100.0, 4.0, vcx),
        50.0
    );
    assert_eq!(length(None, Some("50"), 100.0, 4.0, vcx), 50.0);
    assert_eq!(length(Some(Len::Pct(0.5)), None, 100.0, 4.0, vcx), 50.0);
    assert_eq!(length(None, Some("50%"), 100.0, 4.0, vcx), 50.0);
    assert_eq!(length(Some(Len::Auto), Some("50"), 100.0, 4.0, vcx), 0.0);
    assert_eq!(length(Some(Len::Px(25.0)), None, 100.0, 0.5, vcx), 50.0);
}

#[test]
fn capture_maps_zoomed_css_back_to_svg_units_with_overflow() {
    let style = Computed {
        overflow_x: Some(Overflow::Visible),
        overflow_y: Some(Overflow::Hidden),
        ..Default::default()
    };
    let (origin, size) = capture(&style, (50.0, 25.0), (100.0, 50.0), 4.0);
    assert_eq!(origin, point(px(400.0), px(0.0)));
    assert_eq!(size, gpui::size(px(1000.0), px(100.0)));
    let mut image =
        Element::synthetic("image", Computed::default(), Default::default(), Vec::new());
    expand(&mut image, origin, size, 4.0);
    assert_eq!(image.tag, "svg");
    let Node::Element(child) = &image.children[0] else {
        panic!("image child")
    };
    assert_eq!(child.attr("x"), Some("-100"));
    assert_eq!(child.attr("width"), Some("250"));
    assert_eq!(child.attr("height"), Some("25"));
}
