//! Parsed image operands survive cascade without CSS replay.

use super::*;
use crate::style::{
    cascade::inherit::inherit,
    computed::{GradientShape, RadialSize},
};

#[test]
fn nested_sources_keep_weights_and_filter_program() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let image = ImageSource::parse(
        "cross-fade(red 25%, filter(url(test.png), brightness(50%)))",
        doc.values(),
    )
    .unwrap();
    let ImageSource::CrossFade(items) = image else {
        panic!("cross-fade")
    };
    assert_eq!(
        items.iter().map(|i| i.1).collect::<Vec<_>>(),
        vec![0.25, 0.75]
    );
    let ImageSource::Filter { image, program } = &items[1].0 else {
        panic!("filter")
    };
    assert_eq!(**image, ImageSource::Url("test.png".into()));
    assert_eq!(program.filter.unwrap().brightness, 0.5);
}

#[test]
fn border_radial_lengths_and_current_color_are_computed() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one("color", "green", vcx);
    own.apply_one(
        "border-image-source",
        "radial-gradient(ellipse 2em 3em at 1em 1lh, currentcolor 1em, blue calc(50% + 2lh))",
        vcx,
    );
    let mut c = inherit(&Computed::default(), &own, vcx);
    let image = c.border_image.as_mut().unwrap().src.as_mut().unwrap();
    let ImageSource::Gradient { value, .. } = image else {
        panic!("gradient")
    };
    assert_eq!(value.stops_raw[0].2, Some(20.0));
    assert_eq!(
        (value.stops_raw[1].1, value.stops_raw[1].2),
        (Some(0.5), Some(80.0))
    );
    assert_eq!(value.from, Color::parse("green", vcx).unwrap());
    let GradientShape::Radial { position, size, .. } = &value.shape else {
        panic!("radial")
    };
    assert_eq!(position.x, Some(Len::Px(20.0)));
    assert_eq!(position.y, Some(Len::Px(40.0)));
    assert_eq!(
        *size,
        RadialSize::Explicit(Len::Px(40.0), Some(Len::Px(60.0)))
    );
    image.resolve_lengths(
        &|length| {
            if matches!(length, Some(Len::Em(_))) {
                *length = Some(Len::Px(999.0));
            }
        },
        vcx,
    );
    let (gradient, _) = image.gradient().unwrap();
    assert_eq!(gradient.stops_raw[0].2, Some(20.0));
    assert!(image.cacheable());
}

#[test]
fn conic_retains_angular_hints_and_authored_polar_hues() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let parse = |hue| {
        ImageSource::parse(&format!("conic-gradient(from 90deg at left top in lch decreasing hue, lch(50% 100% {hue}deg), 25%, lch(50% 100% 270deg))"), vcx).unwrap()
    };
    let ImageSource::Conic(a) = parse(0) else {
        panic!("conic")
    };
    let ImageSource::Conic(b) = parse(360) else {
        panic!("conic")
    };
    assert_eq!(a.from, 0.25);
    assert_eq!(a.hints, vec![(1, 0.25)]);
    assert_eq!(a.position.x, Some(Len::Pct(0.0)));
    assert_eq!(a.position.y, Some(Len::Pct(0.0)));
    assert_eq!(a.space, crate::style::computed::GradSpace::Lch);
    assert_eq!(a.hue, 3);
    for t in [0.0, 0.25, 0.5, 1.0] {
        assert_eq!(
            a.polar.as_ref().unwrap().colour_at(&a.stops, t, a.hue),
            b.polar.as_ref().unwrap().colour_at(&b.stops, t, b.hue)
        );
    }
}

#[test]
fn arena_dependent_geometry_does_not_enter_global_cache() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let image = ImageSource::parse(
        "radial-gradient(at calc(50% + 3px) 50%, red, blue)",
        doc.values(),
    )
    .unwrap();
    assert!(!image.cacheable());
}
