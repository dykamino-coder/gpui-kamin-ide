//! KaminIDE patch: what changed inside external textures (browser frames)
//! since the last planned frame, in window pixels.
//!
//! The scene comparison sees a sprite of an external texture as the same
//! entry while its producer repaints the pixels behind it. A producer that
//! reports the rectangle it repainted (`DirectXAtlas::damage_external`) makes
//! the texture tracked; its sprites then add only that rectangle, mapped
//! through the sprite's tile and bounds, to the frame damage. Chromium does
//! the same for an embedded surface: the aggregator maps the child frame's
//! damage rect through the quad's transform into the parent (`SurfaceAggregator`).
//! A texture that never reported damage, or one sampled inside a group or a
//! retained layer (whose cached pixels would hide the change), keeps the frame
//! whole.

use std::collections::{HashMap, HashSet};

use gpui::{Bounds, DevicePixels, PolychromeSprite, Scene, SceneDamage, TransformationMatrix};

/// Filtering margin: a scaled sprite samples neighbouring texels, so pixels
/// next to the mapped rectangle can change too.
const MARGIN: f32 = 2.0;

#[derive(Default)]
pub(crate) struct ExternalDamage {
    /// Polychrome texture indices whose producer reports damage.
    tracked: HashSet<u32>,
    /// Repainted rectangle of each tracked texture, in texture pixels.
    pending: HashMap<u32, Bounds<DevicePixels>>,
}

impl ExternalDamage {
    /// The producer repainted `rect` (texture pixels) of texture `index`.
    pub(crate) fn mark(&mut self, index: u32, rect: Bounds<DevicePixels>) {
        self.tracked.insert(index);
        let next = match self.pending.get(&index) {
            Some(prev) => prev.union(&rect),
            None => rect,
        };
        self.pending.insert(index, next);
    }

    /// The texture is gone or was replaced: nothing is known about it.
    pub(crate) fn forget(&mut self, index: u32) {
        self.tracked.remove(&index);
        self.pending.remove(&index);
    }

    /// Damage that the external textures sampled by `scene` add to the frame;
    /// `is_external` tells which texture indices are external. Consumes the
    /// pending rectangles: the next frame starts from this one.
    pub(crate) fn take(&mut self, scene: &Scene, is_external: impl Fn(u32) -> bool) -> SceneDamage {
        let nested = scene.groups.iter().any(|g| samples(&g.scene, &is_external))
            || (0..scene.layer_count())
                .filter_map(|i| scene.layer(i))
                .any(|layer| samples(layer.scene(), &is_external));
        let mut damage = SceneDamage::Unchanged;
        for sprite in &scene.polychrome_sprites {
            let index = sprite.tile.texture_id.index;
            if !is_external(index) {
                continue;
            }
            damage = damage.union(if self.tracked.contains(&index) {
                self.pending
                    .get(&index)
                    .map_or(SceneDamage::Unchanged, |rect| map(*rect, sprite))
            } else {
                SceneDamage::Unknown
            });
        }
        self.pending.clear();
        if nested {
            SceneDamage::Unknown
        } else {
            damage
        }
    }
}

fn samples(scene: &Scene, is_external: &impl Fn(u32) -> bool) -> bool {
    scene
        .polychrome_sprites
        .iter()
        .any(|s| is_external(s.tile.texture_id.index))
        || scene.groups.iter().any(|g| samples(&g.scene, is_external))
        || (0..scene.layer_count())
            .filter_map(|i| scene.layer(i))
            .any(|layer| samples(layer.scene(), is_external))
}

/// `rect` (texture pixels) as window pixels the sprite may repaint.
fn map(rect: Bounds<DevicePixels>, sprite: &PolychromeSprite) -> SceneDamage {
    let tile = sprite.tile.bounds;
    let hit = rect.intersect(&tile);
    if hit.size.width.0 <= 0 || hit.size.height.0 <= 0 {
        return SceneDamage::Unchanged;
    }
    let b = sprite.bounds;
    let (mut x0, mut y0, mut x1, mut y1) = if sprite.transformation
        == TransformationMatrix::unit()
        && tile.size.width.0 > 0
        && tile.size.height.0 > 0
    {
        let sx = b.size.width.0 / tile.size.width.0 as f32;
        let sy = b.size.height.0 / tile.size.height.0 as f32;
        let x0 = b.origin.x.0 + (hit.origin.x.0 - tile.origin.x.0) as f32 * sx;
        let y0 = b.origin.y.0 + (hit.origin.y.0 - tile.origin.y.0) as f32 * sy;
        (
            x0 - MARGIN,
            y0 - MARGIN,
            x0 + hit.size.width.0 as f32 * sx + MARGIN,
            y0 + hit.size.height.0 as f32 * sy + MARGIN,
        )
    } else {
        // A transformed sprite: its whole bounds (the mask still clips it).
        (
            b.origin.x.0,
            b.origin.y.0,
            b.origin.x.0 + b.size.width.0,
            b.origin.y.0 + b.size.height.0,
        )
    };
    let m = sprite.content_mask.bounds;
    x0 = x0.max(m.origin.x.0).max(0.0);
    y0 = y0.max(m.origin.y.0).max(0.0);
    x1 = x1.min(m.origin.x.0 + m.size.width.0);
    y1 = y1.min(m.origin.y.0 + m.size.height.0);
    if x1 <= x0 || y1 <= y0 {
        return SceneDamage::Unchanged;
    }
    let (x, y) = (x0.floor() as u32, y0.floor() as u32);
    SceneDamage::Rect {
        x,
        y,
        width: (x1.ceil() as u32).saturating_sub(x).max(1),
        height: (y1.ceil() as u32).saturating_sub(y).max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AtlasTextureId, AtlasTextureKind, AtlasTile, ContentMask, Point, ScaledPixels, Size, TileId};

    fn dev(x: i32, y: i32, w: i32, h: i32) -> Bounds<DevicePixels> {
        Bounds {
            origin: Point { x: DevicePixels(x), y: DevicePixels(y) },
            size: Size { width: DevicePixels(w), height: DevicePixels(h) },
        }
    }

    fn scaled(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: Point { x: ScaledPixels(x), y: ScaledPixels(y) },
            size: Size { width: ScaledPixels(w), height: ScaledPixels(h) },
        }
    }

    /// A 400×300 page tile drawn 1:1 at (100, 50).
    fn page(index: u32) -> PolychromeSprite {
        PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale: false.into(),
            opacity: 1.0,
            bounds: scaled(100.0, 50.0, 400.0, 300.0),
            content_mask: ContentMask { bounds: scaled(0.0, 0.0, 2000.0, 2000.0) },
            corner_radii: Default::default(),
            tile: AtlasTile {
                texture_id: AtlasTextureId { index, kind: AtlasTextureKind::Polychrome },
                tile_id: TileId(0),
                padding: 0,
                bounds: dev(0, 0, 400, 300),
            },
            transformation: TransformationMatrix::unit(),
        }
    }

    #[test]
    fn a_reported_rect_maps_into_the_window_with_a_margin() {
        let mut d = ExternalDamage::default();
        d.mark(7, dev(10, 20, 2, 16));
        assert_eq!(
            map(dev(10, 20, 2, 16), &page(7)),
            SceneDamage::Rect { x: 108, y: 68, width: 6, height: 20 }
        );
    }

    #[test]
    fn a_rect_outside_the_tile_or_mask_damages_nothing() {
        assert_eq!(map(dev(500, 0, 10, 10), &page(7)), SceneDamage::Unchanged);
        let mut s = page(7);
        s.content_mask.bounds = scaled(0.0, 0.0, 50.0, 50.0);
        assert_eq!(map(dev(0, 0, 10, 10), &s), SceneDamage::Unchanged);
    }

    #[test]
    fn a_scaled_tile_scales_the_rect() {
        let mut s = page(7);
        s.bounds = scaled(0.0, 0.0, 800.0, 600.0);
        assert_eq!(
            map(dev(100, 100, 10, 10), &s),
            SceneDamage::Rect { x: 198, y: 198, width: 24, height: 24 }
        );
    }
}
