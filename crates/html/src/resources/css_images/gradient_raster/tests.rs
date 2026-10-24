//! Попадания числового кэша, различимость операндов и детерминизм NaN.

use super::*;
use crate::style::computed::ImageSource;

#[test]
fn numeric_images_hit_across_documents_and_keep_uncached_pixels() {
    for css in [
        "linear-gradient(red 3px, blue)",
        "radial-gradient(at 25% 40%, red, blue)",
        "conic-gradient(from 30deg at 20% 60%, red, 25%, blue)",
        "linear-gradient(in lch, lch(50% 50 20), lch(70% 40 280))",
        "image(green)",
    ] {
        let a = crate::document::state::DocState::new(crate::style::css::Media::default());
        let b = crate::document::state::DocState::new(crate::style::css::Media::default());
        let first = ImageSource::parse(css, a.values()).unwrap();
        let second = ImageSource::parse(css, b.values()).unwrap();
        let cached = tile(&first, 13, 7, (12.75, 7.25), a.values()).unwrap();
        let hit = tile(&second, 13, 7, (12.75, 7.25), b.values()).unwrap();
        assert!(Arc::ptr_eq(&cached, &hit), "{css}");
        let direct = raster(&second, 13, 7, (12.75, 7.25), true, b.values()).unwrap();
        assert_eq!(cached.bytes, direct.bytes, "{css}");
    }
}

#[test]
fn equal_hints_do_not_hide_gradient_changes() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut source = ImageSource::parse("linear-gradient(red, blue)", vcx).unwrap();
    let red = tile(&source, 11, 9, (11.0, 9.0), vcx).unwrap();
    let ImageSource::Gradient { value, .. } = &mut source else {
        panic!("gradient")
    };
    value.stops_raw[0].0 = crate::style::values::value::Color::BLACK;
    let black = tile(&source, 11, 9, (11.0, 9.0), vcx).unwrap();
    assert!(!Arc::ptr_eq(&red, &black));
    assert_ne!(red.bytes, black.bytes);
}

#[test]
fn nan_payloads_and_signed_zero_have_repeatable_rasters_and_hits() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for values in [[0.0, -0.0], [f32::NAN, f32::from_bits(0xff80_0123)]] {
        let mut rasters = Vec::new();
        for angle in values {
            let mut source = ImageSource::parse("linear-gradient(red, blue)", vcx).unwrap();
            let ImageSource::Gradient { value, .. } = &mut source else {
                panic!("gradient")
            };
            value.angle_deg = angle;
            let direct = raster(&source, 8, 6, (8.0, 6.0), true, vcx).unwrap();
            let cached = tile(&source, 8, 6, (8.0, 6.0), vcx).unwrap();
            assert_eq!(direct.bytes, cached.bytes);
            rasters.push(cached);
        }
        assert!(Arc::ptr_eq(&rasters[0], &rasters[1]));
    }
}
