//! Допуск по заявленным размерам: граница каждого счётчика, крошечный файл с огромным
//! растром, общий physical буфер нескольких документов, вытеснение при живом `Arc`.

use super::*;
use crate::budget::{BudgetExceeded, BudgetScope, DocumentLimits, ProcessLimits, ResourceGovernor};
use crate::cache::MIB;
use crate::resources::images::cache::DecodedImageCache;
use crate::resources::images::fixtures::{declared_png, png};
use crate::resources::images::types::{DecodeOptions, DecodedImage};

fn document(governor: &ResourceGovernor, kind: BudgetKind, limit: u64) -> DocumentBudget {
    let limits = DocumentLimits::default().lower(kind, limit).unwrap();
    DocumentBudget::new(limits, governor.clone()).unwrap()
}

fn exceeded(error: &ImageError) -> Option<BudgetExceeded> {
    match error {
        ImageError::LimitExceeded(ImageLimit::Budget(BudgetError::Exceeded(e))) => Some(*e),
        _ => None,
    }
}

fn rgb_png(w: u32, h: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            &vec![9; (w * h * 3) as usize],
            w,
            h,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    out
}

#[test]
fn declared_pixels_boundary_is_checked_before_decoding() {
    let bytes: Arc<[u8]> = png(4, 4).into();
    for (limit, ok) in [(15, false), (16, true), (17, true)] {
        let governor = ResourceGovernor::default();
        let doc = document(&governor, BudgetKind::DecodedPixels, limit);
        let cache = DecodedImageCache::new(MIB);
        let (decoded, admission) = cache.decode(bytes.clone(), DecodeOptions::default(), &doc);
        assert_eq!(decoded.is_ok(), ok);
        assert_eq!(doc.used(BudgetKind::DecodedPixels), if ok { 16 } else { 0 });
        if !ok {
            let e = exceeded(decoded.as_ref().unwrap_err()).unwrap();
            assert_eq!(
                (e.scope, e.limit, e.increment),
                (BudgetScope::Document, 15, 16)
            );
            assert_eq!(
                cache.len(),
                0,
                "a quota refusal is not a property of the bytes"
            );
        }
        assert_eq!(
            doc.used(BudgetKind::LiveBytes),
            0,
            "temporaries end with decoding"
        );
        drop((decoded, admission, cache));
        assert_eq!(governor.used(BudgetKind::DecodedPixels), 0);
        assert_eq!(doc.used(BudgetKind::DecodedPixels), 0);
    }
}

#[test]
fn tiny_compressed_huge_declared_raster_is_refused_without_decoding() {
    let bytes = declared_png(60_000, 60_000);
    assert!(bytes.len() < 128);
    let governor = ResourceGovernor::default();
    let doc = document(&governor, BudgetKind::Documents, 1);
    let cache = DecodedImageCache::new(MIB);
    let (decoded, _) = cache.decode(bytes.into(), DecodeOptions::default(), &doc);
    // Отказ квоты, а не ошибка кодека: точки не выделялись и не декодировались.
    let e = exceeded(&decoded.unwrap_err()).unwrap();
    assert_eq!(e.kind, BudgetKind::DecodedPixels);
    assert_eq!(e.increment, 3_600_000_000);
    assert_eq!(e.scope, BudgetScope::Process);
    assert_eq!(governor.used(BudgetKind::DecodedPixels), 0);
}

#[test]
fn conversion_temporaries_are_live_bytes_during_decoding() {
    // RGB8 4×4: выход кодека 48 байт живёт рядом с RGBA-копией.
    let bytes: Arc<[u8]> = rgb_png(4, 4).into();
    for (limit, ok) in [(47, false), (48, true), (49, true)] {
        let governor = ResourceGovernor::default();
        let doc = document(&governor, BudgetKind::LiveBytes, limit);
        let cache = DecodedImageCache::new(MIB);
        let (decoded, _) = cache.decode(bytes.clone(), DecodeOptions::default(), &doc);
        assert_eq!(decoded.is_ok(), ok);
        if !ok {
            assert_eq!(
                exceeded(decoded.as_ref().unwrap_err()).unwrap().increment,
                48
            );
            assert_eq!(governor.used(BudgetKind::DecodedPixels), 0, "rolled back");
        }
        assert_eq!(doc.used(BudgetKind::LiveBytes), 0);
    }
    let overflow = Declared {
        width: u32::MAX,
        height: u32::MAX,
        native: 0,
        converts: false,
        orients: true,
    };
    assert!(matches!(
        overflow.temporary(),
        Err(ImageError::LimitExceeded(ImageLimit::Dimensions { .. }))
    ));
}

#[test]
fn documents_share_one_physical_raster_and_eviction_keeps_live_owners() {
    let bytes: Arc<[u8]> = png(4, 4).into();
    let governor = ResourceGovernor::default();
    let (a, b) = (
        document(&governor, BudgetKind::Documents, 1),
        document(&governor, BudgetKind::Documents, 1),
    );
    let cache = DecodedImageCache::new(MIB);
    let options = DecodeOptions::default();
    let (first, held_a) = cache.decode(bytes.clone(), options, &a);
    let (second, held_b) = cache.decode(bytes, options, &b);
    let raster = |d: &crate::resources::images::cache::Decoded| match d {
        Ok((DecodedImage::Raster(r), _)) => r.clone(),
        _ => panic!("raster expected"),
    };
    assert!(Arc::ptr_eq(&raster(&first), &raster(&second)));
    assert_eq!(cache.len(), 1);
    assert_eq!(
        governor.used(BudgetKind::DecodedPixels),
        16,
        "one physical buffer"
    );
    assert_eq!(
        (
            a.used(BudgetKind::DecodedPixels),
            b.used(BudgetKind::DecodedPixels)
        ),
        (16, 16)
    );
    let live = raster(&second);
    drop((first, second, held_a, cache));
    assert_eq!(a.used(BudgetKind::DecodedPixels), 0);
    assert_eq!(
        governor.used(BudgetKind::DecodedPixels),
        16,
        "evicted, still owned"
    );
    drop(held_b);
    assert_eq!(b.used(BudgetKind::DecodedPixels), 0);
    assert_eq!(
        governor.used(BudgetKind::DecodedPixels),
        16,
        "the raster Arc is alive"
    );
    drop(live);
    assert_eq!(governor.used(BudgetKind::DecodedPixels), 0);
}

#[test]
fn other_governors_decode_their_own_copy_under_their_own_quota() {
    let bytes: Arc<[u8]> = png(4, 4).into();
    let roomy = ResourceGovernor::default();
    let tight = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::DecodedPixels, 15)
            .unwrap(),
    );
    let cache = DecodedImageCache::new(MIB);
    let options = DecodeOptions::default();
    let ok = document(&roomy, BudgetKind::Documents, 1);
    let (decoded, _held) = cache.decode(bytes.clone(), options, &ok);
    assert!(decoded.is_ok());
    let refused = document(&tight, BudgetKind::Documents, 1);
    let (decoded, _) = cache.decode(bytes, options, &refused);
    let e = exceeded(&decoded.unwrap_err()).unwrap();
    assert_eq!((e.scope, e.limit), (BudgetScope::Process, 15));
    assert_eq!(roomy.used(BudgetKind::DecodedPixels), 16);
    assert_eq!(tight.used(BudgetKind::DecodedPixels), 0);
}
