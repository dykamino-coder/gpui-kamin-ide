//! Видимые сплошные квады, растровые заливки и глифы сцены в порядке краски, с номером
//! группы эффекта; границы после матрицы примитива.

use gpui::{Bounds, Pixels, ScaledPixels, Scene, point, px, size};

/// Device bounds of `q` under `m` (four corners).
fn placed(q: Bounds<ScaledPixels>, m: gpui::TransformationMatrix) -> Bounds<ScaledPixels> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in [
        (q.origin.x.0, q.origin.y.0),
        (q.right().0, q.origin.y.0),
        (q.origin.x.0, q.bottom().0),
        (q.right().0, q.bottom().0),
    ] {
        let p = m.apply(point(px(x), px(y)));
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    Bounds::new(
        point(ScaledPixels(x0), ScaledPixels(y0)),
        size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
    )
}

pub(super) fn collect(scene: &Scene, scale: f32, out: &mut Vec<(Bounds<Pixels>, gpui::Hsla, usize)>) {
    let mut pending = vec![(scene, 0)];
    let mut groups = 0;
    while let Some((scene, group)) = pending.pop() {
        for quad in &scene.quads {
            // Non-solid fills (gradients) report a negative hue.
            let color = match quad.background.as_solid() {
                Some(c) if c.a > 0. => c,
                Some(_) => continue,
                None => gpui::hsla(-1., 0., 0., 1.),
            };
            let b = placed(quad.bounds, quad.transformation);
            let visible = b.intersect(&quad.content_mask.bounds);
            if visible.size.width.0 <= 0. || visible.size.height.0 <= 0. {
                continue;
            }
            let logical = Bounds::new(
                point(
                    px(visible.origin.x.0 / scale),
                    px(visible.origin.y.0 / scale),
                ),
                size(
                    px(visible.size.width.0 / scale),
                    px(visible.size.height.0 / scale),
                ),
            );
            out.push((logical, color, group));
        }
        // Glyphs report as near-opaque black quads (text presence).
        for glyph in &scene.monochrome_sprites {
            let b = placed(glyph.bounds, glyph.transformation).intersect(&glyph.content_mask.bounds);
            if b.size.width.0 > 0. && b.size.height.0 > 0. {
                let logical = Bounds::new(
                    point(px(b.origin.x.0 / scale), px(b.origin.y.0 / scale)),
                    size(px(b.size.width.0 / scale), px(b.size.height.0 / scale)),
                );
                out.push((logical, gpui::hsla(0., 0., 0., 0.999), group));
            }
        }
        // Raster fills (images, rasterized gradients) report hue -2.
        for sprite in &scene.polychrome_sprites {
            let b = placed(sprite.bounds, sprite.transformation).intersect(&sprite.content_mask.bounds);
            if b.size.width.0 > 0. && b.size.height.0 > 0. {
                let logical = Bounds::new(
                    point(px(b.origin.x.0 / scale), px(b.origin.y.0 / scale)),
                    size(px(b.size.width.0 / scale), px(b.size.height.0 / scale)),
                );
                out.push((logical, gpui::hsla(-2., 0., 0., 1.), group));
            }
        }
        for inner in scene.groups.iter().rev() {
            groups += 1;
            pending.push((&inner.scene, groups));
        }
    }
}
