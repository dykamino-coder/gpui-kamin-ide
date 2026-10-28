//! Воспроизведение записи под произвольной аффинной презентацией с паритетом live-отрисовки:
//! матрица композируется в примитивы так же, как `Window::with_transformation`, а маски
//! переносятся только при сохранении осей, как у `Window::with_transformation_masked`.

use super::*;
use crate::TransformationMatrix;

/// The rule of `with_transformation_masked`: masks move only with an axis-keeping matrix.
pub(crate) fn keeps_axes(m: &TransformationMatrix) -> bool {
    let r = m.rotation_scale;
    (r[0][1].abs() < 1e-4 && r[1][0].abs() < 1e-4) || (r[0][0].abs() < 1e-4 && r[1][1].abs() < 1e-4)
}

/// Device bounds of the image of `b` (four corners).
pub(crate) fn placed(b: Bounds<ScaledPixels>, m: &TransformationMatrix) -> Bounds<ScaledPixels> {
    let corners = [
        point(b.origin.x, b.origin.y), point(b.right(), b.origin.y),
        point(b.origin.x, b.bottom()), point(b.right(), b.bottom()),
    ];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for corner in corners {
        let p = map(corner, m);
        x0 = x0.min(p.x.0);
        y0 = y0.min(p.y.0);
        x1 = x1.max(p.x.0);
        y1 = y1.max(p.y.0);
    }
    Bounds::new(point(ScaledPixels(x0), ScaledPixels(y0)), crate::size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)))
}

fn map(p: Point<ScaledPixels>, m: &TransformationMatrix) -> Point<ScaledPixels> {
    let p = m.apply(point(crate::px(p.x.0), crate::px(p.y.0)));
    point(ScaledPixels(p.x.0), ScaledPixels(p.y.0))
}

impl Scene {
    /// `matrix` is a non-singular device-space affine transform.
    pub(crate) fn transform_affine(mut self, matrix: TransformationMatrix, inverse: TransformationMatrix) -> Self {
        let masks = keeps_axes(&matrix);
        let groups = std::mem::take(&mut self.groups);
        let mut scene = self.affine_commands(&matrix, masks);
        scene.groups = groups.into_iter().map(|mut group| {
            group.bounds = placed(group.bounds, &matrix);
            group.polygon.iter_mut().for_each(|p| *p = map(*p, &matrix));
            if masks {
                group.mask_bounds = placed(group.mask_bounds, &matrix);
                group.mask_clip = group.mask_clip.map(|b| {
                    let b = placed(Bounds::new(point(ScaledPixels(b[0]), ScaledPixels(b[1])),
                        crate::size(ScaledPixels(b[2]), ScaledPixels(b[3]))), &matrix);
                    [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0]
                });
            }
            group.projection = group.projection.map(|h| super::vector_transform::conjugate(h, matrix, inverse));
            for fragment in &mut group.fragments {
                fragment.iter_mut().for_each(|p| *p = map(*p, &matrix));
            }
            group.scene = group.scene.affine_commands(&matrix, masks);
            group
        }).collect();
        scene
    }

    fn affine_commands(mut self, matrix: &TransformationMatrix, masks: bool) -> Self {
        let mut scene = Self { in_group: self.in_group, ..Self::default() };
        let mask = |bounds: &mut Bounds<ScaledPixels>| if masks { *bounds = placed(*bounds, matrix) };
        for mut command in std::mem::take(&mut self.paint_operations) {
            match &mut command {
                PaintOperation::Primitive(p) => match p {
                    // Live paint carries the active matrix on these primitives only.
                    Primitive::Quad(p) => {
                        p.transformation = matrix.compose(p.transformation);
                        mask(&mut p.content_mask.bounds);
                    }
                    Primitive::MonochromeSprite(p) => {
                        p.transformation = matrix.compose(p.transformation);
                        mask(&mut p.content_mask.bounds);
                    }
                    Primitive::SubpixelSprite(p) => {
                        p.transformation = matrix.compose(p.transformation);
                        mask(&mut p.content_mask.bounds);
                    }
                    Primitive::PolychromeSprite(p) => {
                        p.transformation = matrix.compose(p.transformation);
                        mask(&mut p.content_mask.bounds);
                    }
                    Primitive::Shadow(p) => mask(&mut p.content_mask.bounds),
                    Primitive::Underline(p) => mask(&mut p.content_mask.bounds),
                    Primitive::Surface(p) => mask(&mut p.content_mask.bounds),
                    Primitive::Path(p) => {
                        mask(&mut p.content_mask.bounds);
                        p.vertices.iter_mut().for_each(|v| mask(&mut v.content_mask.bounds));
                    }
                },
                // Layer bounds are placed like live layers under a matrix.
                PaintOperation::StartLayer(b) => *b = placed(*b, matrix),
                _ => {}
            }
            scene.replay_operation(command);
        }
        scene.segment_capture = self.segment_capture;
        scene.hoisted_backdrops = self.hoisted_backdrops.into_iter().map(|mut p| {
            mask(&mut p.content_mask.bounds);
            p
        }).collect();
        scene.finish();
        scene
    }
}
