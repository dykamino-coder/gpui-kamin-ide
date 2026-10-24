//! Тесты: вычисленные шрифтовые длины градиента сохраняют проценты и унаследованные базы.

use crate::style::computed::{BoxStyle, Computed, GradientShape, RadialSize};
use crate::style::values::value::Len;

#[test]
fn mixed_font_stops_compute_once_before_inheritance() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one(
        "background-image",
        "linear-gradient(currentcolor 1em 2em, blue calc(50% + 3em))",
        vcx,
    );
    let parent = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    let mut own = Computed::default();
    own.apply_one("font-size", "80px", vcx);
    own.apply_one("background-image", "inherit", vcx);
    own.apply_one("color", "green", vcx);
    let child = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
    let gradient = child.gradient.clone().unwrap();
    assert!(gradient.stop_lengths.is_empty());
    assert_eq!(
        gradient
            .stops_raw
            .iter()
            .map(|s| (s.1, s.2))
            .collect::<Vec<_>>(),
        vec![
            (None, Some(20.0)),
            (None, Some(40.0)),
            (Some(0.5), Some(60.0))
        ]
    );
}

#[test]
fn radial_radii_center_and_line_stops_are_computed_lengths() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one(
        "background-image",
        "radial-gradient(ellipse 2em 3em at 1em 50%, red 1lh, blue calc(50% + 2lh))",
        vcx,
    );
    let computed = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    let gradient = computed.gradient.clone().unwrap();
    let GradientShape::Radial { position, size, .. } = gradient.shape else {
        panic!("radial gradient")
    };
    assert_eq!(position.x, Some(Len::Px(20.0)));
    assert_eq!(position.y, Some(Len::Pct(0.5)));
    assert_eq!(
        size,
        RadialSize::Explicit(Len::Px(40.0), Some(Len::Px(60.0)))
    );
    assert_eq!(gradient.stops_raw[0].2, Some(40.0));
    assert_eq!(
        (gradient.stops_raw[1].1, gradient.stops_raw[1].2),
        (Some(0.5), Some(80.0))
    );
    assert!(gradient.stop_lengths.is_empty());
}

#[test]
fn viewport_and_font_terms_share_the_computed_stop() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one(
        "background-image",
        "linear-gradient(red calc(10vw + 2em), blue calc(50% + 3vh))",
        vcx,
    );
    let mut computed = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    computed.resolve_viewport((200.0, 100.0), vcx);
    computed.resolve_em(16.0, vcx);
    let gradient = computed.gradient.clone().unwrap();
    assert_eq!(
        (gradient.stops_raw[0].1, gradient.stops_raw[0].2),
        (None, Some(60.0))
    );
    assert_eq!(
        (gradient.stops_raw[1].1, gradient.stops_raw[1].2),
        (Some(0.5), Some(3.0))
    );
    assert!(gradient.stop_lengths.is_empty());
}
