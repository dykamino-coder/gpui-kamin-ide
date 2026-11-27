use super::*;
use crate::{AtlasKey, HeadlessAtlas, PlatformAtlas, RenderImageParams, Scene};
use std::borrow::Cow;

fn image() -> Arc<RenderImage> {
    crate::bgra_bytes_to_image(2, 2, (1..=16).collect()).unwrap()
}

#[test]
fn large_derived_images_keep_no_cpu_pixels_and_do_not_pin_sources() {
    let source = image();
    let weak = Arc::downgrade(&source);
    let mut cache = Cache::default();
    let derived = resample(&mut cache, &source, 0, 4096, 4096, [0.25; 2], 1.25).unwrap();
    assert!(derived.as_bytes(0).is_none());
    assert_eq!(Arc::strong_count(&source), 1);
    drop(source);
    assert!(weak.upgrade().is_none());
    cache.release_unused();
    assert!(cache.entries.is_empty());
    assert!(derived.upload_bytes(0).is_none());
}

#[test]
fn lazy_upload_preserves_exact_source_phase_and_window_independence() {
    let source = image();
    let (mut a, mut b) = (Cache::default(), Cache::default());
    for scale in [0.75, 1.0, 1.25, 2.0] {
        for phase in [[0.25, -0.5], [-0.125, 0.5]] {
            let x = resample(&mut a, &source, 0, 3, 5, phase, scale).unwrap();
            let y = resample(&mut b, &source, 0, 3, 5, phase, scale).unwrap();
            assert_ne!(x.id, y.id);
            let mut expected = Vec::new();
            for row in 0..5 {
                for col in 0..3 {
                    let at = ((sample(row, 2, phase[1], scale) * 2
                        + sample(col, 2, phase[0], scale))
                        * 4) as usize;
                    expected.extend_from_slice(&source.as_bytes(0).unwrap()[at..at + 4]);
                }
            }
            assert_eq!(x.upload_bytes(0).unwrap().as_ref(), expected);
            assert_eq!(x.upload_bytes(0).unwrap(), y.upload_bytes(0).unwrap());
            let hit = resample(&mut a, &source, 0, 3, 5, phase, scale).unwrap();
            assert!(Arc::ptr_eq(&x, &hit));
        }
    }
}

#[test]
fn cache_evicts_one_least_recent_identity_instead_of_clearing_all() {
    let source = image();
    let mut cache = Cache::default();
    let mut weak = Vec::new();
    for w in 3..35 {
        let derived = resample(&mut cache, &source, 0, w, 5, [0.25; 2], 1.25).unwrap();
        weak.push(Arc::downgrade(&derived));
    }
    resample(&mut cache, &source, 0, 3, 5, [0.25; 2], 1.25).unwrap();
    resample(&mut cache, &source, 0, 35, 5, [0.25; 2], 1.25).unwrap();
    assert_eq!(cache.entries.len(), CAPACITY);
    assert!(weak[0].upgrade().is_some());
    assert!(weak[1].upgrade().is_none());
    assert!(weak[2..].iter().all(|entry| entry.upgrade().is_some()));
}

#[test]
fn atlas_reuse_rasters_once_then_navigation_releases_cache_and_upload() {
    let instrumentation = crate::Instrumentation::new(crate::InstrumentationConfig {
        gpu_timing: true,
        ..Default::default()
    });
    let mut cache = Cache::new(&instrumentation);
    let mut uploads = super::super::image_resources::ImageUploads::default();
    let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
    let source = image();
    let mut previous = Scene::default();
    for _ in 0..3 {
        let derived = resample(&mut cache, &source, 0, 3, 5, [0.25; 2], 1.25).unwrap();
        let key: AtlasKey = RenderImageParams {
            image_id: derived.id,
            frame_index: 0,
        }
        .into();
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    derived.size(0),
                    Cow::Owned(derived.upload_bytes(0).unwrap().into_owned()),
                )))
            })
            .unwrap()
            .unwrap();
        let mut next = Scene::default();
        next.keep_image(tile, uploads.lease(&atlas, &derived, 0));
        previous.clear();
        previous = next;
        assert!(atlas.contains(&key));
    }
    let line = instrumentation.batches.take_line().unwrap();
    assert!(line.contains("natural_image_create=1 natural_image_raster=1 natural_image_raster_bytes=60 natural_image_drop=0"), "{line}");
    let key: AtlasKey = RenderImageParams {
        image_id: cache.entries[0].image.id,
        frame_index: 0,
    }
    .into();
    drop(source);
    cache.release_unused();
    uploads.release_unused();
    assert!(cache.entries.is_empty());
    assert!(
        atlas.contains(&key),
        "the retained scene still owns its tile"
    );
    previous.clear();
    assert!(!atlas.contains(&key));
    assert!(
        instrumentation
            .batches
            .take_line()
            .unwrap()
            .contains("natural_image_drop=1")
    );
    assert!(instrumentation.batches.take_line().is_none());
}

#[test]
fn disabled_journal_is_absent_from_cache_and_lazy_upload() {
    let instrumentation = crate::Instrumentation::default();
    let mut cache = Cache::new(&instrumentation);
    assert!(cache.journal.is_none());
    let source = image();
    let derived = resample(&mut cache, &source, 0, 3, 3, [0.25; 2], 1.25).unwrap();
    assert!(derived.upload_bytes(0).is_some());
    drop(source);
    cache.release_unused();
    assert!(instrumentation.batches.take_line().is_none());
}
