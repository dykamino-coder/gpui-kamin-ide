//! Atlas images live until both their cache entry and last scene release them.

use super::{Primitive, Scene};
use crate::{AtlasKey, AtlasTextureId, AtlasTile, PlatformAtlas};
use collections::FxHashMap;
use std::{
    rc::Rc,
    sync::{Arc, Weak},
};

pub(crate) type ImageResources = FxHashMap<(AtlasTextureId, u32), Rc<ImageLease>>;

pub(crate) struct ImageLease {
    atlas: Weak<dyn PlatformAtlas>,
    key: AtlasKey,
}

impl ImageLease {
    pub(crate) fn new(atlas: &Arc<dyn PlatformAtlas>, key: AtlasKey) -> Rc<Self> {
        Rc::new(Self {
            atlas: Arc::downgrade(atlas),
            key,
        })
    }
}

impl Drop for ImageLease {
    fn drop(&mut self) {
        if let Some(atlas) = self.atlas.upgrade() {
            atlas.remove(&self.key);
        }
    }
}

impl Scene {
    pub(crate) fn keep_image(&mut self, tile: AtlasTile, lease: Rc<ImageLease>) {
        self.book
            .images
            .entry((tile.texture_id, tile.tile_id.0))
            .or_insert(lease);
    }

    pub(super) fn copy_image(&mut self, primitive: &Primitive, source: &ImageResources) {
        if let Primitive::PolychromeSprite(sprite) = primitive {
            let key = (sprite.tile.texture_id, sprite.tile.tile_id.0);
            if let Some(lease) = source.get(&key) {
                self.book.images.entry(key).or_insert_with(|| lease.clone());
            }
        }
    }

    pub(super) fn insert_image_primitive(&mut self, primitive: Primitive, source: &ImageResources) {
        let tile = match &primitive {
            Primitive::PolychromeSprite(sprite) => Some(sprite.tile),
            _ => None,
        };
        let before = self.paint_operations.len();
        self.insert_primitive(primitive);
        if self.paint_operations.len() > before
            && let Some(tile) = tile
            && let Some(lease) = source.get(&(tile.texture_id, tile.tile_id.0))
        {
            self.keep_image(tile, lease.clone());
        }
    }

    pub(super) fn replay_image_operation(
        &mut self,
        operation: super::PaintOperation,
        source: &ImageResources,
    ) {
        let before = self.paint_operations.len();
        self.replay_operation(self.relabel_owned(operation));
        if self.paint_operations.len() > before
            && let Some(super::PaintOperation::Primitive(Primitive::PolychromeSprite(sprite))) =
                self.paint_operations.last()
            && let Some(lease) = source.get(&(sprite.tile.texture_id, sprite.tile.tile_id.0))
        {
            self.keep_image(sprite.tile, lease.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Bounds, ContentMask, HeadlessAtlas, ImageId, Point, PolychromeSprite, RenderImageParams,
        ScaledPixels, TransformationMatrix, point, size,
    };
    use std::borrow::Cow;

    fn uploaded(
        atlas: &Arc<dyn PlatformAtlas>,
        id: usize,
    ) -> (AtlasTile, Rc<ImageLease>, AtlasKey) {
        let key: AtlasKey = RenderImageParams {
            image_id: ImageId(id),
            frame_index: 0,
        }
        .into();
        let tile = atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((
                    size(crate::DevicePixels(2), crate::DevicePixels(2)),
                    Cow::Owned(vec![255; 16]),
                )))
            })
            .unwrap()
            .unwrap();
        (tile, ImageLease::new(atlas, key.clone()), key)
    }

    fn sprite(tile: AtlasTile, x: f32) -> PolychromeSprite {
        let bounds = Bounds::new(
            point(ScaledPixels(x), ScaledPixels(0.)),
            size(ScaledPixels(10.), ScaledPixels(10.)),
        );
        PolychromeSprite {
            pattern: crate::Size::default(),
            bounds,
            content_mask: ContentMask { bounds },
            tile,
            transformation: TransformationMatrix::unit(),
            order: 0,
            pad: 0,
            grayscale: false.into(),
            opacity: 1.,
            corner_radii: Default::default(),
        }
    }

    fn source(atlas: &Arc<dyn PlatformAtlas>) -> (Scene, Vec<AtlasKey>) {
        let mut scene = Scene::default();
        let mut keys = Vec::new();
        for id in 1..=2 {
            let (tile, lease, key) = uploaded(atlas, id);
            scene.insert_primitive(sprite(tile, (id - 1) as f32 * 100.));
            scene.keep_image(tile, lease);
            keys.push(key);
        }
        scene.finish();
        (scene, keys)
    }

    #[test]
    fn atlas_image_lives_until_last_cache_or_scene_owner() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (tile, cached, key) = uploaded(&atlas, 42);
        let mut old = Scene::default();
        old.insert_primitive(sprite(tile, 0.));
        old.keep_image(tile, cached.clone());
        let mut next = Scene::default();
        next.replay(0..old.len(), &old);
        drop(cached);
        old.clear();
        assert!(
            atlas.contains(&key),
            "the replayed frame still needs the tile"
        );
        next.clear();
        assert!(!atlas.contains(&key), "the final scene releases the tile");
    }

    #[test]
    fn clipped_copy_does_not_pin_unpainted_images() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (source, keys) = source(&atlas);
        let clip = Bounds::new(Point::default(), size(ScaledPixels(20.), ScaledPixels(20.)));
        let mut copy = source.copy_record(Point::default(), Some(clip));
        drop(source);
        assert!(atlas.contains(&keys[0]));
        assert!(!atlas.contains(&keys[1]));
        copy.clear();
        assert!(!atlas.contains(&keys[0]));
    }

    #[test]
    fn placed_replay_keeps_only_visible_images() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (source, keys) = source(&atlas);
        let clip = Bounds::new(Point::default(), size(ScaledPixels(20.), ScaledPixels(20.)));
        let mut output = Scene::default();
        assert!(output.replay_placed(&source, Point::default(), Some(clip)));
        drop(source);
        assert!(atlas.contains(&keys[0]));
        assert!(!atlas.contains(&keys[1]));
        drop(output);
        assert!(!atlas.contains(&keys[0]));
    }

    #[test]
    fn transformed_and_cow_scenes_retain_tiles() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (source, keys) = source(&atlas);
        let layer = crate::SceneLayer::finish(source, Vec::new(), Vec::new());
        let copy = layer.clone();
        let identity = TransformationMatrix::unit();
        let transformed = layer.scene.transform_affine(identity, identity);
        drop(copy);
        assert!(keys.iter().all(|key| atlas.contains(key)));
        drop(transformed);
        assert!(keys.iter().all(|key| !atlas.contains(key)));
    }

    #[test]
    fn bottom_layer_replay_fallback_retains_tiles() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
        let (tile, lease, key) = uploaded(&atlas, 77);
        let mut source = Scene::default();
        source.push_bottom_layer();
        source.insert_primitive(sprite(tile, 0.));
        source.keep_image(tile, lease);
        source.pop_layer();
        let mut output = Scene::default();
        output.replay_record(&source);
        drop(source);
        assert!(atlas.contains(&key));
        drop(output);
        assert!(!atlas.contains(&key));
    }
}
