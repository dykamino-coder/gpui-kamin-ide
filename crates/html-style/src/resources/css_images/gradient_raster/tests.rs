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
        let a = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let b = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let first = ImageSource::parse(css, a.values()).unwrap();
        let second = ImageSource::parse(css, b.values()).unwrap();
        let cached = tile(&first, 13, 7, (12.75, 7.25), a.values()).unwrap();
        let hit = tile(&second, 13, 7, (12.75, 7.25), b.values()).unwrap();
        assert!(Arc::ptr_eq(&cached, &hit), "{css}");
        let direct = uncached(&second, 13, 7, (12.75, 7.25), true, b.values()).unwrap();
        assert_eq!(cached.bytes, direct.bytes, "{css}");
    }
}

#[test]
fn equal_hints_do_not_hide_gradient_changes() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
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
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for values in [[0.0, -0.0], [f32::NAN, f32::from_bits(0xff80_0123)]] {
        let mut rasters = Vec::new();
        for angle in values {
            let mut source = ImageSource::parse("linear-gradient(red, blue)", vcx).unwrap();
            let ImageSource::Gradient { value, .. } = &mut source else {
                panic!("gradient")
            };
            value.angle_deg = angle;
            let direct = uncached(&source, 8, 6, (8.0, 6.0), true, vcx).unwrap();
            let cached = tile(&source, 8, 6, (8.0, 6.0), vcx).unwrap();
            assert_eq!(direct.bytes, cached.bytes);
            rasters.push(cached);
        }
        assert!(Arc::ptr_eq(&rasters[0], &rasters[1]));
    }
}

#[test]
fn legacy_edge_and_device_center_sampling_have_separate_cache_entries() {
    let doc = crate::values::DocumentValues::detached(Default::default());
    let src = ImageSource::parse("linear-gradient(90deg, red, blue)", doc.values()).unwrap();
    let edges = raster(&src, 11, 7, (11.0, 7.0), false, doc.values()).unwrap();
    let centers = raster(&src, 11, 7, (11.0, 7.0), true, doc.values()).unwrap();
    assert!(!Arc::ptr_eq(&edges, &centers));
    assert_ne!(edges.bytes, centers.bytes);
    assert_eq!(
        edges.bytes,
        uncached(&src, 11, 7, (11.0, 7.0), false, doc.values())
            .unwrap()
            .bytes
    );
}

/// The DevTools `why` journal names a tile lookup the cache cannot serve:
/// a source with document lengths (`uncacheable`), a key that differs from
/// a seen one (`new_key`; its field is `gradient_why`'s test). `no_key`
/// guards a cacheable source the key cannot describe: every source that
/// `cacheable` admits has a key today, so no lookup reaches it.
#[test]
fn tile_misses_name_their_reason() {
    let (_one, _sub) = crate::journal::test_reader();
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let relative = ImageSource::parse("radial-gradient(at 1em 2em, red, blue)", vcx).unwrap();
    assert!(!relative.cacheable());
    tile(&relative, 5, 5, (5.0, 5.0), vcx);
    let plain = ImageSource::parse("linear-gradient(red, #123457)", vcx).unwrap();
    tile(&plain, 17, 3, (17.0, 3.0), vcx);
    tile(&plain, 18, 3, (18.0, 3.0), vcx);
    let j = crate::journal::take();
    let tiles = &j.caches[CACHE];
    let reasons: Vec<_> = tiles
        .examples
        .iter()
        .map(|e| (e.code, e.detail.as_str()))
        .collect();
    assert!(
        reasons
            .iter()
            .any(|(r, d)| *r == "uncacheable" && d.contains("5x5")),
        "{reasons:?}"
    );
    // Examples keep the first misses of each reason; tests in parallel add theirs.
    assert!(tiles.misses["new_key"] >= 2, "{reasons:?}");
}
