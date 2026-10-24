//! Хранилище документа над общим кэшем: два документа с одним адресом и разной базой или
//! политикой, кэш по содержимому, дедупликация, изменение файла и отрицательный итог,
//! отписка, обновление документа и устаревший итог, вытеснение без потери живых `Arc`.

use super::*;
use crate::cache::MIB;
use crate::resources::images::cache::DecodedImageCache;
use crate::resources::images::tests_support::{TempDir, data_png_url, pixel, png_bytes, png_sized};
use crate::resources::images::types::CpuRaster;
use crate::resources::loader::SyncLoader;
use std::sync::Arc;

fn services(budget: usize) -> ImageServices {
    ImageServices {
        loader: Arc::new(SyncLoader),
        cache: Arc::new(DecodedImageCache::new(budget)),
    }
}

fn store(services: &ImageServices, dir: &TempDir, policy: ResourcePolicy) -> ImageStore {
    let base = Url::from_file_path(dir.path().join("page.html")).unwrap();
    ImageStore::new(services.clone(), Some(base), policy)
}

fn ask(store: &mut ImageStore, authored: &str) -> ImageId {
    let (url, _) = store.resolve(authored);
    store.request(ImageRequest {
        url,
        options: DecodeOptions::default(),
    })
}

fn raster(store: &ImageStore, id: ImageId) -> Arc<CpuRaster> {
    match store.snapshot().state(id) {
        Some(ImageState::Ready {
            image: DecodedImage::Raster(r),
            ..
        }) => r.clone(),
        other => panic!("raster expected, got {other:?}"),
    }
}

#[test]
fn two_documents_resolve_one_url_by_their_own_base_and_policy() {
    let (mut a, mut b) = (TempDir::new("doc-a"), TempDir::new("doc-b"));
    a.write("img.png", &png_bytes([255, 0, 0, 255]));
    b.write("img.png", &png_bytes([0, 0, 255, 255]));
    let shared = services(MIB);
    let mut first = store(&shared, &a, ResourcePolicy::default());
    let mut second = store(&shared, &b, ResourcePolicy::default());
    let mut locked = store(&shared, &a, ResourcePolicy::data_only());
    let x = ask(&mut first, "img.png");
    let y = ask(&mut second, "img.png");
    let z = ask(&mut locked, "img.png");
    assert!(first.has_pending());
    assert!(!locked.has_pending());
    // Access is decided per document before any IO, whatever the cache holds.
    assert!(matches!(
        locked.snapshot().state(z),
        Some(ImageState::Failed(ImageError::AccessDenied))
    ));
    assert!(locked.take_jobs().is_empty());
    first.complete_pending();
    assert!(!first.has_pending());
    second.complete_pending();
    assert_eq!(pixel(&raster(&first, x)), [255, 0, 0, 255]);
    assert_eq!(pixel(&raster(&second, y)), [0, 0, 255, 255]);
}

#[test]
fn equal_bytes_under_different_urls_share_one_decoded_value() {
    let mut dir = TempDir::new("content-key");
    let png = png_bytes([1, 2, 3, 255]);
    dir.write("a.png", &png);
    dir.write("b.png", &png);
    let shared = services(MIB);
    let mut doc = store(&shared, &dir, ResourcePolicy::default());
    let (a, b) = (ask(&mut doc, "a.png"), ask(&mut doc, "b.png"));
    assert_ne!(a, b);
    // One request per document; a fragment selects a view, it is not another fetch.
    assert_eq!(ask(&mut doc, "a.png#view"), a);
    assert_eq!(ask(&mut doc, "a.png"), a);
    doc.complete_pending();
    assert!(Arc::ptr_eq(&raster(&doc, a), &raster(&doc, b)));
    assert_eq!(shared.cache.len(), 1);
}

#[test]
fn file_changes_and_negative_entries_are_revalidated() {
    let mut dir = TempDir::new("revalidate");
    let mut doc = store(&services(MIB), &dir, ResourcePolicy::default());
    let id = ask(&mut doc, "late.png");
    doc.complete_pending();
    assert!(matches!(
        doc.snapshot().state(id),
        Some(ImageState::Failed(ImageError::Io))
    ));
    let before = doc.snapshot().revision();
    assert_eq!(doc.revalidate(), 0, "still absent");
    dir.write("late.png", &png_bytes([0, 255, 0, 255]));
    assert_eq!(
        doc.revalidate(),
        1,
        "a file that appeared is not missing forever"
    );
    let changes = doc.complete_pending();
    assert!(changes[0].terminal && changes[0].pixels_changed && changes[0].metadata_changed);
    assert_eq!(pixel(&raster(&doc, id)), [0, 255, 0, 255]);
    assert!(doc.snapshot().revision() > before);
    assert_eq!(doc.revalidate(), 0, "unchanged file");
    // A different length is a different revision whatever the clock resolution.
    dir.write("late.png", &png_sized(2, 1, [9, 9, 9, 255]));
    assert_eq!(doc.revalidate(), 1);
    let changes = doc.complete_pending();
    assert!(changes[0].metadata_changed && changes[0].pixels_changed);
    assert_eq!(raster(&doc, id).width, 2);
}

#[test]
fn release_update_and_stale_completions_never_resurrect_requests() {
    let data = data_png_url();
    let shared = services(MIB);
    let mut doc = ImageStore::new(shared.clone(), None, ResourcePolicy::default());
    let id = ask(&mut doc, &data);
    let sub = doc
        .subscribe(id, ConsumerId(7), ImageUse::Intrinsic)
        .unwrap();
    let alpha = doc.subscribe(id, ConsumerId(8), ImageUse::Alpha).unwrap();
    doc.release(alpha);
    assert!(
        doc.snapshot().state(id).is_some(),
        "another consumer still holds it"
    );
    let jobs = doc.take_jobs();
    assert!(doc.has_pending(), "выданное задание ещё не декодировано");
    doc.release(sub);
    assert!(
        doc.snapshot().state(id).is_none(),
        "released handle is stale"
    );
    assert!(doc.apply_completed(jobs[0].run(doc.services())).is_none());
    // The freed slot is reused under a new generation: the old handle does not alias it.
    let again = ask(&mut doc, &data);
    assert_ne!(again, id);
    let jobs = doc.take_jobs();
    doc.begin_generation();
    assert!(
        doc.apply_completed(jobs[0].run(&shared)).is_none(),
        "previous generation"
    );
    assert!(doc.snapshot().state(again).is_none());
    // A superseded attempt is ignored; the current one notifies its consumers.
    let fresh = ask(&mut doc, &data);
    doc.subscribe(fresh, ConsumerId(9), ImageUse::Facts)
        .unwrap();
    let old = doc.take_jobs();
    doc.slots[fresh.slot as usize]
        .entry
        .as_mut()
        .unwrap()
        .content = Some(Revision::Absent);
    assert_eq!(doc.revalidate(), 1);
    assert!(
        doc.apply_completed(old[0].run(&shared)).is_none(),
        "superseded attempt"
    );
    let changes = doc.complete_pending();
    assert_eq!(changes[0].consumers, vec![(ConsumerId(9), ImageUse::Facts)]);
    assert!(changes[0].terminal);
}

#[test]
fn budget_eviction_keeps_live_values() {
    let mut dir = TempDir::new("evict");
    dir.write("a.png", &png_bytes([10, 0, 0, 255]));
    dir.write("b.png", &png_bytes([20, 0, 0, 255]));
    // Each entry holds 1 KiB of pixels: the budget keeps exactly one.
    let tight = services(1800);
    let mut doc = store(&tight, &dir, ResourcePolicy::default());
    let a = ask(&mut doc, "a.png");
    doc.subscribe(a, ConsumerId(1), ImageUse::Pixels).unwrap();
    doc.complete_pending();
    let held = raster(&doc, a);
    let b = ask(&mut doc, "b.png");
    doc.complete_pending();
    assert_eq!(tight.cache.len(), 1, "the older entry was evicted");
    assert_eq!(pixel(&held), [10, 0, 0, 255]);
    assert!(
        Arc::ptr_eq(&held, &raster(&doc, a)),
        "the document keeps its value"
    );
    assert_eq!(pixel(&raster(&doc, b)), [20, 0, 0, 255]);
}
