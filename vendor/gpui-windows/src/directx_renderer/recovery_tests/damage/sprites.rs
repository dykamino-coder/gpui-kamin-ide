//! KaminIDE patch: a colour sprite draws the same pixels wherever the atlas
//! holds its tile. A frame after an edit lays the atlas out anew, so a
//! sampler that resolved normalized atlas coordinates gave a rotated or scaled
//! image other filter weights there and the frame differed from a fresh one.

use super::*;
use gpui::{
    AtlasKey, AtlasTile, Bounds, ContentMask, PlatformAtlas, Point,
    PolychromeSprite, Radians, RenderImageParams, ScaledPixels, TransformationMatrix,
};
use std::borrow::Cow;

const W: i32 = 23;
const H: i32 = 17;

/// A premultiplied BGRA image whose every texel differs from its neighbours.
fn texels() -> Vec<u8> {
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let a = (128 + (x * 5 + y * 3) % 128) as u8;
            let c = |k: i32| ((x * k + y * (k + 7)) % i32::from(a)) as u8;
            out.extend_from_slice(&[c(11), c(29), c(53), a]);
        }
    }
    out
}

fn insert(r: &DirectXRenderer, id: usize, w: i32, h: i32, bytes: Vec<u8>) -> AtlasTile {
    let size = Size { width: DevicePixels(w), height: DevicePixels(h) };
    let key = AtlasKey::Image(RenderImageParams { image_id: ImageId(id), frame_index: 0 });
    let mut bytes = Some(bytes);
    r.atlas
        .get_or_insert_with(key, &mut || Ok(Some((size, Cow::Owned(bytes.take().unwrap())))))
        .unwrap()
        .unwrap()
}

fn scaled(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: Point { x: ScaledPixels(x), y: ScaledPixels(y) },
        size: Size { width: ScaledPixels(w), height: ScaledPixels(h) },
    }
}

/// The image drawn rotated and scaled, and once 1:1 on the device grid.
fn frame(r: &mut DirectXRenderer, tile: AtlasTile) -> Vec<u8> {
    let mut scene = Scene::default();
    let turn = TransformationMatrix::unit()
        .translate(Point { x: ScaledPixels(30.0), y: ScaledPixels(22.0) })
        .rotate(Radians(0.37))
        .translate(Point { x: ScaledPixels(-30.0), y: ScaledPixels(-22.0) });
    for (bounds, transformation) in [
        (scaled(12.3, 9.6, 41.5, 29.0), turn),
        (scaled(4.0, 40.0, W as f32, H as f32), TransformationMatrix::unit()),
    ] {
        scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale: false.into(),
            opacity: 1.0,
            bounds,
            content_mask: ContentMask { bounds: scaled(0.0, 0.0, 64.0, 64.0) },
            corner_radii: Default::default(),
            tile,
            transformation,
        });
    }
    scene.finish();
    r.render_to_image(&scene, WindowBackgroundAppearance::Opaque).unwrap().into_raw()
}

#[test]
fn a_colour_sprite_draws_the_same_pixels_at_any_atlas_place() {
    use crate::GpuAdapterPreference as P;
    for (p, name) in [(P::Default, "default"), (P::Software, "software")] {
        let Ok(device) = DirectXDevices::with_instrumentation(p, gpui::Instrumentation::default()) else {
            continue;
        };
        let mut first = offscreen(&device, 64, 64);
        let a = insert(&first, 1, W, H, texels());
        let mut second = offscreen(&device, 64, 64);
        // Another image first: the same image lands elsewhere in the atlas.
        insert(&second, 2, 37, 29, vec![255; 37 * 29 * 4]);
        let b = insert(&second, 1, W, H, texels());
        assert_ne!(a.bounds.origin, b.bounds.origin, "{name}: the layouts must differ");
        let (fa, fb) = (frame(&mut first, a), frame(&mut second, b));
        let differ = fa.chunks(4).zip(fb.chunks(4)).filter(|(x, y)| x != y).count();
        assert_eq!(differ, 0, "{name}: {differ} pixels depend on the atlas place");
    }
}
