//! CPU-кэш и сцены совместно удерживают загрузки растра до последнего использования.

use crate::scene::image_resources::ImageLease;
use crate::{PlatformAtlas, RenderImage, RenderImageParams};
use collections::FxHashMap;
use std::{
    rc::Rc,
    sync::{Arc, Weak},
};

#[derive(Default)]
pub(super) struct ImageUploads(FxHashMap<RenderImageParams, Upload>);

struct Upload {
    image: Weak<RenderImage>,
    lease: Rc<ImageLease>,
}

impl ImageUploads {
    pub(super) fn lease(
        &mut self,
        atlas: &Arc<dyn PlatformAtlas>,
        image: &Arc<RenderImage>,
        frame_index: usize,
    ) -> Rc<ImageLease> {
        let key = RenderImageParams {
            image_id: image.id,
            frame_index,
        };
        let upload = self.0.entry(key.clone()).or_insert_with(|| Upload {
            image: Arc::downgrade(image),
            lease: ImageLease::new(atlas, key.into()),
        });
        upload.lease.clone()
    }

    pub(super) fn release_unused(&mut self) {
        // Слабая ссылка разрешает CPU-кэшу вытеснить пиксели. Сохранённые
        // и replay-сцены держат загрузку до конца последнего нарисованного кадра.
        self.0.retain(|_, upload| upload.image.strong_count() > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtlasKey, HeadlessAtlas, Scene};
    use std::borrow::Cow;

    fn upload(
        uploads: &mut ImageUploads,
        atlas: &Arc<dyn PlatformAtlas>,
        image: &Arc<RenderImage>,
        scene: &mut Scene,
    ) -> AtlasKey {
        let key: AtlasKey = RenderImageParams {
            image_id: image.id,
            frame_index: 0,
        }
        .into();
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    image.size(0),
                    Cow::Borrowed(image.as_bytes(0).unwrap()),
                )))
            })
            .unwrap()
            .unwrap();
        scene.keep_image(tile, uploads.lease(atlas, image, 0));
        key
    }

    #[test]
    fn cpu_cache_and_scene_share_one_upload_owner() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let mut uploads = ImageUploads::default();
        let image = crate::bgra_bytes_to_image(2, 2, vec![255; 16]).unwrap();
        let mut old = Scene::default();
        let key = upload(&mut uploads, &atlas, &image, &mut old);
        let mut next = Scene::default();
        upload(&mut uploads, &atlas, &image, &mut next);
        assert_eq!(uploads.0.len(), 1);
        old.clear();
        uploads.release_unused();
        assert!(atlas.contains(&key));
        drop(image);
        uploads.release_unused();
        assert!(atlas.contains(&key));
        next.clear();
        assert!(!atlas.contains(&key));
    }

    #[test]
    fn raster_churn_is_bounded_by_live_frames_and_cpu_images() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let mut uploads = ImageUploads::default();
        let mut previous = Scene::default();
        let mut expired = Vec::new();
        for _ in 0..12 {
            let mut next = Scene::default();
            let mut keys = Vec::new();
            for _ in 0..80 {
                let image = crate::bgra_bytes_to_image(2, 2, vec![255; 16]).unwrap();
                keys.push(upload(&mut uploads, &atlas, &image, &mut next));
            }
            uploads.release_unused();
            assert!(uploads.0.is_empty());
            previous.clear();
            assert!(expired.iter().all(|key| !atlas.contains(key)));
            assert!(keys.iter().all(|key| atlas.contains(key)));
            expired = keys;
            previous = next;
        }
        previous.clear();
        assert!(expired.iter().all(|key| !atlas.contains(key)));
    }

    #[test]
    fn separate_atlases_release_their_own_uploads() {
        let a: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let b: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (mut ac, mut bc) = (ImageUploads::default(), ImageUploads::default());
        let image = crate::bgra_bytes_to_image(2, 2, vec![255; 16]).unwrap();
        let (mut sa, mut sb) = (Scene::default(), Scene::default());
        let key = upload(&mut ac, &a, &image, &mut sa);
        upload(&mut bc, &b, &image, &mut sb);
        drop(image);
        ac.release_unused();
        bc.release_unused();
        sa.clear();
        assert!(!a.contains(&key));
        assert!(b.contains(&key));
        sb.clear();
        assert!(!b.contains(&key));
    }

    #[test]
    fn clipped_upload_without_scene_owner_is_collected() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let mut uploads = ImageUploads::default();
        let image = crate::bgra_bytes_to_image(2, 2, vec![255; 16]).unwrap();
        let mut scene = Scene::default();
        let key = upload(&mut uploads, &atlas, &image, &mut scene);
        scene.clear();
        uploads.release_unused();
        assert!(atlas.contains(&key), "the CPU cache still owns the image");
        drop(image);
        uploads.release_unused();
        assert!(!atlas.contains(&key));
    }
}
