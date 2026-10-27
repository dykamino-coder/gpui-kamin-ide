//! Isolated transparent scene capture for embedded document content.
use super::*;
use image::ImageEncoder as _;

impl Window {
    /// Lay out embedded content at its capture density without changing display state.
    pub fn with_scene_capture_scale<R>(
        &mut self,
        density: f32,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.scale_factor;
        self.scale_factor *= density.max(1.0);
        self.with_restore(previous, |window, previous| {
            window.scale_factor = *previous;
        }, f).0
    }

    /// Paint a local scene into a transparent PNG, without presenting a frame.
    /// The caller lays out and prepaints its content in the same local coordinates.
    pub fn paint_scene_to_png(
        &mut self,
        size: Size<Pixels>,
        density: f32,
        paint: impl FnOnce(&mut Self),
    ) -> anyhow::Result<Vec<u8>> {
        self.invalidator.debug_assert_paint();
        let sf = self.scale_factor() * density.max(1.0);
        let (_, mut scene) = self.with_isolated_scene(|window| {
            let previous = (
                mem::replace(&mut window.scale_factor, sf),
                window.mask_scale.take(),
                mem::replace(&mut window.element_opacity, 1.0),
            );
            window.with_restore(previous, |window, previous| {
                window.scale_factor = previous.0;
                window.mask_scale = previous.1;
                window.element_opacity = previous.2;
            }, |window| {
                window.with_absolute_transformation(TransformationMatrix::unit(), |window| {
                    window.with_content_mask_replaced(
                        ContentMask { bounds: Bounds { origin: Point::default(), size } },
                        paint,
                    );
                });
            });
        });
        normalize_solid_quads(&mut scene);
        scene.finish();
        let size = crate::size(
            DevicePixels((f32::from(size.width) * sf).ceil().max(1.0) as i32),
            DevicePixels((f32::from(size.height) * sf).ceil().max(1.0) as i32),
        );
        let mut image = self.platform_window.rasterize_scene(&scene, size)?;
        // GPU scenes contain premultiplied RGBA; PNG stores straight RGBA.
        for pixel in image.pixels_mut() {
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel.0[..3] {
                    *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )?;
        Ok(bytes)
    }
}

// Bake uniform scale into solid geometry so the quad shader's antialiasing
// stays in capture pixels, regardless of whether scale came from HTML or SVG.
fn normalize_solid_quads(scene: &mut Scene) {
    for quad in &mut scene.quads {
        let [[s, b], [c, d]] = quad.transformation.rotation_scale;
        if s <= 0.0
            || !s.is_finite()
            || s != d
            || b != 0.0
            || c != 0.0
            || quad.background.tag != crate::BackgroundTag::Solid
            || quad.border_style != crate::BorderStyle::Solid
        {
            continue;
        }
        let [x, y] = quad.transformation.translation;
        quad.bounds.origin.x = ScaledPixels(quad.bounds.origin.x.0 * s + x);
        quad.bounds.origin.y = ScaledPixels(quad.bounds.origin.y.0 * s + y);
        quad.bounds.size.width = quad.bounds.size.width * s;
        quad.bounds.size.height = quad.bounds.size.height * s;
        quad.corner_radii = quad.corner_radii.map(|v| *v * s);
        quad.border_widths = quad.border_widths.map(|v| *v * s);
        quad.transformation = TransformationMatrix::unit();
    }
    for group in &mut scene.groups {
        normalize_solid_quads(&mut group.scene);
    }
}
