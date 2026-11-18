//! Pixel compatibility, shared ownership and resource lifetime.

use super::upload;
use crate::resources::images::{fixtures, types::*};
use std::sync::Arc;

#[test]
fn straight_and_premultiplied_match_legacy_upload() {
    let rgba = vec![8, 16, 24, 128, 7, 9, 11, 0, 3, 5, 7, 255];
    for alpha in [AlphaMode::Straight, AlphaMode::Premultiplied] {
        let cpu = Arc::new(CpuRaster {
            width: 3,
            height: 1,
            stride: 12,
            format: PixelFormat::Rgba8,
            alpha,
            colour_space: ColourSpace::Srgb,
            bytes: rgba.clone().into(),
            retention: Default::default(),
        });
        let mut expected = rgba.clone();
        for p in expected.chunks_exact_mut(4) {
            if alpha == AlphaMode::Premultiplied {
                gpui::swap_rgba_pa_to_bgra(p);
            } else {
                p.swap(0, 2);
            }
        }
        let image = upload(&cpu).unwrap();
        assert_eq!(image.as_bytes(0).unwrap(), expected);
        assert!(Arc::ptr_eq(&image, &upload(&cpu).unwrap()));
    }
}

#[test]
fn translucent_png_icc_golden() {
    let png = fixtures::with_chunk(
        &fixtures::solid_png([8, 16, 24, 128]),
        b"iCCP",
        &fixtures::iccp(&fixtures::linear_srgb_profile()),
    );
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let source =
        crate::resources::css_images::source(&fixtures::data_url("image/png", &png), doc.values())
            .unwrap();
    let cpu = source.raster((1.0, 1.0), doc.values()).unwrap();
    let image = upload(&cpu).unwrap();
    assert_eq!(image.as_bytes(0).unwrap(), [82, 68, 48, 128]);
    assert_eq!(cpu.bytes.as_ptr(), image.as_bytes(0).unwrap().as_ptr());
    let downloaded = super::download(&image).unwrap();
    assert!(Arc::ptr_eq(&cpu.bytes, &downloaded.bytes));
    assert!(Arc::ptr_eq(&cpu, &downloaded));
    assert!(Arc::ptr_eq(&downloaded, &super::download(&image).unwrap()));
}

#[test]
fn svg_cpu_matches_previous_gpui_rasterizer() {
    let store = crate::resources::images::store::ImageStore::default();
    for body in [
        "<rect width='9' height='7' fill='#214365' opacity='.37'/>",
        "<circle cx='4.5' cy='3.5' r='3' fill='red'/>",
        "<rect width='11' height='9' fill='#214365' opacity='.37'/>",
        "<defs><linearGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue' stop-opacity='.3'/></linearGradient></defs><rect width='9' height='7' fill='url(#g)'/>",
        "",
    ] {
        let markup =
            format!("<svg xmlns='http://www.w3.org/2000/svg' width='9' height='7'>{body}</svg>");
        for (w, h, density) in [(9.0, 7.0, 1.0), (10.4, 6.7, 2.0), (3.0, 5.0, 1.5)] {
            let old = gpui::svg_markup_to_image(&markup, w, h, density).unwrap();
            let cpu = store
                .rasterize_svg(
                    &markup,
                    crate::resources::images::svg_raster::RasterRequest::new(w, h, density, 0.0),
                )
                .unwrap();
            assert_eq!(
                upload(&cpu).unwrap().as_bytes(0),
                old.as_bytes(0),
                "{body} {density}"
            );
            let old = gpui::svg_markup_to_image_padded(&markup, w, h, density, 1.5);
            let cpu = store.rasterize_svg(
                &markup,
                crate::resources::images::svg_raster::RasterRequest::new(w, h, density, 1.5),
            );
            assert_eq!(
                cpu.as_ref()
                    .and_then(upload)
                    .and_then(|image| image.as_bytes(0).map(<[u8]>::to_vec)),
                old.as_ref()
                    .and_then(|image| image.as_bytes(0).map(<[u8]>::to_vec)),
                "padded {body} {density}"
            );
        }
    }
}

#[test]
fn independent_documents_share_fractional_svg_edge_upload() {
    use crate::resources::images::{store::ImageStore, svg_raster::RasterRequest};
    // css-skew-001: the same fractional edge in different documents.
    // With different ImageIds, linear atlas sampling at (88, 61) produced
    // RGB (116, 186, 116) and (117, 186, 117), even comparing a reference to itself.
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' width='231.642' height='268.778'><path d='M 231.64212,268.77841 73.167806,158.54816 0,0 158.58452,110.15942 z' fill='green'/></svg>";
    let first = ImageStore::default();
    let second = ImageStore::default();
    let request = RasterRequest::new(231.642, 268.778, 2.0, 0.0);
    let a = first.rasterize_svg(markup, request).unwrap();
    let b = second.rasterize_svg(markup, request).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!(a.bytes, b.bytes);
    let legacy = gpui::svg_markup_to_image(markup, 231.642, 268.778, 2.0).unwrap();
    assert_eq!(a.bytes.as_ref(), legacy.as_bytes(0).unwrap());
    // Neighbours sampled at centre (88.5, 61.5) in a 463 x 538 raster.
    let edge =
        [(176, 122), (177, 122), (176, 123), (177, 123)].map(|(x, y)| a.pixel(x, y).unwrap());
    assert_eq!(
        edge,
        [
            [0, 128, 0, 64],
            [0, 128, 0, 0],
            [0, 128, 0, 255],
            [0, 128, 0, 127]
        ]
    );
    let image_a = upload(&a).unwrap();
    let image_b = upload(&b).unwrap();
    assert!(Arc::ptr_eq(&image_a, &image_b));
}

#[test]
fn upload_identity_includes_extent_and_every_pixel() {
    use crate::resources::images::pixels::bgra;
    let bytes = vec![0, 128, 0, 64, 0, 128, 0, 127];
    let a = bgra(2, 1, bytes.clone()).unwrap();
    let b = bgra(1, 2, bytes.clone()).unwrap();
    let mut changed = bytes;
    changed[7] = 128;
    let c = bgra(2, 1, changed).unwrap();
    let image = upload(&a).unwrap();
    assert_ne!(image.id, upload(&b).unwrap().id);
    assert_ne!(image.id, upload(&c).unwrap().id);
}

#[test]
fn shared_render_image_retains_physical_pixel_grant_until_drop() {
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
    use crate::resources::images::cache::DecodedImageCache;
    let governor = ResourceGovernor::default();
    let doc = DocumentBudget::new(DocumentLimits::default(), governor.clone()).unwrap();
    let cache = DecodedImageCache::new(0);
    let (decoded, admission) = cache.decode(
        fixtures::solid_png([1, 2, 3, 255]).into(),
        DecodeOptions {
            colour: ColourPolicy::CssBgra,
            ..Default::default()
        },
        &doc,
    );
    let (DecodedImage::Raster(cpu), _) = decoded.unwrap() else {
        panic!("raster")
    };
    let image = gpui::RenderImage::from_shared_bgra(1, 1, cpu.clone()).unwrap();
    assert!(Arc::ptr_eq(&cpu, &super::download(&image).unwrap()));
    drop((cpu, admission, cache, doc));
    assert_eq!(governor.used(BudgetKind::DecodedPixels), 1);
    drop(image);
    assert_eq!(governor.used(BudgetKind::DecodedPixels), 0);
}

#[test]
fn repeated_downloads_reuse_the_same_resampled_variant() {
    use crate::resources::images::{pixels::bgra, resample::resample};
    let cpu = bgra(2, 1, vec![0, 128, 0, 255, 255, 0, 0, 0]).unwrap();
    let image = upload(&cpu).unwrap();
    let first = super::download(&image).unwrap();
    let resized = resample(&first, 4, 1).unwrap();
    let second = super::download(&image).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&resized, &resample(&second, 4, 1).unwrap()));
    assert_eq!(
        resized.bytes.as_ref(),
        &[0, 128, 0, 255, 0, 128, 0, 191, 0, 128, 0, 64, 0, 0, 0, 0]
    );
}
