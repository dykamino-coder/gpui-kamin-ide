//! Тесты: разобранные операнды изображений проходят каскад без повторного разбора CSS.

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

#[test]
fn background_layers_inherit_computed_image_lengths_and_current_colors() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one("background-image", "conic-gradient(at 1lh 1em, currentcolor, blue), linear-gradient(currentcolor 1em, blue 2em)", vcx);
    let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
    let mut own = Computed::default();
    own.apply_one("background", "inherit", vcx);
    own.apply_one("font-size", "80px", vcx);
    own.apply_one("color", "green", vcx);
    let child = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
    let layers = child.bg_layers(vcx).unwrap();
    let ImageSource::Conic(image) = layers[0].bg_image.as_ref().unwrap() else {
        panic!("conic");
    };
    assert_eq!(image.position.x, Some(Len::Px(40.0)));
    assert_eq!(image.position.y, Some(Len::Px(20.0)));
    assert_eq!(image.stops[0].0, Color::parse("green", vcx).unwrap());
    let image = layers[1].bg_image.as_ref().unwrap().gradient().unwrap().0;
    assert_eq!(image.stops_px[0].1, 20.0);
    assert_eq!(image.stops_px[1].1, 40.0);
}

#[test]
fn layered_background_geometry_uses_own_line_height() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one("background-image", "url(a), url(b)", vcx);
    own.apply_one("background-size", "1lh 2lh, 3lh 4lh", vcx);
    own.apply_one("background-position", "1lh 2lh, 3lh 4lh", vcx);
    let c = inherit(&Computed::default(), &own, vcx);
    let layers = c.bg_layers(vcx).unwrap();
    assert_eq!(layers[1].bg_pos.x, Some(Len::Px(120.0)));
    assert_eq!(layers[1].bg_pos.y, Some(Len::Px(160.0)));
    assert_eq!(
        layers[0].bg_size,
        super::super::BgSize::Fixed(Some(Len::Px(40.0)), Some(Len::Px(80.0)))
    );
}

#[test]
fn filtered_gradient_stops_keep_interpolation_without_color_replay() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (header, space) in [
        ("", super::super::GradSpace::Srgb),
        ("in oklab, ", super::super::GradSpace::Oklab),
    ] {
        let mut image = ImageSource::parse(
            &format!("repeating-linear-gradient({header}currentcolor, oklab(50% 0.1 0.1))"),
            vcx,
        )
        .unwrap();
        image.resolve_colors(Color::parse("red", vcx).unwrap());
        image.filter_stops(&Filter {
            brightness: 0.5,
            ..Filter::neutral()
        });
        let before = image.gradient().unwrap().0.stops.clone();
        image.resolve_colors(Color::parse("green", vcx).unwrap());
        let value = image.gradient().unwrap().0;
        assert_eq!(value.space, space);
        assert_eq!(value.stops, before);
        assert_eq!(value.stops[0].0.r, 128.0 / 255.0);
    }
}
#[test]
fn url_function_names_keep_css_case_and_escape_rules() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for raw in ["UrL(MixedCase.png)", r#"U\r\4c ("MixedCase.png")"#] {
        let decls = crate::style::css::parse_decls(&format!("background: {raw}"));
        let mut style = Computed::default();
        style.apply_decls(&decls, vcx);
        assert_eq!(
            style.bg_image,
            Some(ImageSource::Url("MixedCase.png".into()))
        );
    }
}
