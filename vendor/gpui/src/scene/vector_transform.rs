//! Точный перенос векторных команд страницы: равномерный масштаб и четверти оборота.

use super::*;
use crate::{PaintVectorTransform, TransformationMatrix};

impl Scene {
    pub(crate) fn transform_vector(mut self, transform: PaintVectorTransform, density: f32) -> Self {
        let matrix = transform.matrix(density);
        let groups = mem::take(&mut self.groups);
        let mut scene = self.transform_commands(transform, matrix);
        scene.groups = groups.into_iter().map(|mut group| {
            group.bounds = map_bounds(group.bounds, matrix);
            group.blur_radius *= transform.scale();
            group.polygon.iter_mut().for_each(|p| *p = map_point(*p, matrix));
            group.mask_bounds = map_bounds(group.mask_bounds, matrix);
            let turn = ((group.mask_once >> 3) + u32::from(transform.quarter_turns())) % 4;
            group.mask_once = (group.mask_once & 7) | (turn << 3);
            group.mask_clip = group.mask_clip.map(|b| {
                let b = map_bounds(Bounds::new(point(ScaledPixels(b[0]), ScaledPixels(b[1])),
                    crate::size(ScaledPixels(b[2]), ScaledPixels(b[3]))), matrix);
                [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0]
            });
            group.projection = group.projection.map(|m| conjugate(m, matrix, transform.inverse_matrix(density)));
            for fragment in &mut group.fragments {
                fragment.iter_mut().for_each(|p| *p = map_point(*p, matrix));
            }
            group.scene = group.scene.transform_commands(transform, matrix);
            group
        }).collect();
        scene
    }

    fn transform_commands(mut self, transform: PaintVectorTransform, matrix: TransformationMatrix) -> Self {
        let mut scene = Self { in_group: self.in_group, ..Self::default() };
        for mut command in mem::take(&mut self.paint_operations) {
            match &mut command {
                PaintOperation::Primitive(p) => primitive(p, transform, matrix),
                PaintOperation::StartLayer(b) => *b = map_bounds(*b, matrix),
                _ => {},
            }
            scene.replay_operation(command);
        }
        scene.segment_capture = self.segment_capture;
        scene.hoisted_backdrops = self.hoisted_backdrops.into_iter().map(|mut p| {
            surface(&mut p, transform, matrix);
            p
        }).collect();
        scene.finish();
        scene
    }
}

use std::mem;

fn map_point(p: Point<ScaledPixels>, m: TransformationMatrix) -> Point<ScaledPixels> {
    let p = m.apply(point(crate::px(p.x.0), crate::px(p.y.0)));
    point(ScaledPixels(p.x.0), ScaledPixels(p.y.0))
}

fn map_bounds(b: Bounds<ScaledPixels>, m: TransformationMatrix) -> Bounds<ScaledPixels> {
    let a = map_point(b.origin, m);
    let z = map_point(b.bottom_right(), m);
    Bounds::new(point(a.x.min(z.x), a.y.min(z.y)),
        crate::size((z.x - a.x).abs(), (z.y - a.y).abs()))
}

fn radii(c: &mut Corners<ScaledPixels>, t: PaintVectorTransform) {
    let mut v = [c.top_left, c.top_right, c.bottom_right, c.bottom_left];
    v.rotate_right(usize::from(t.quarter_turns()));
    *c = Corners { top_left: v[0], top_right: v[1], bottom_right: v[2], bottom_left: v[3] }.map(|x| *x * t.scale());
}

fn surface(p: &mut PaintSurface, t: PaintVectorTransform, m: TransformationMatrix) {
    p.bounds = map_bounds(p.bounds, m);
    p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
    #[cfg(not(target_os = "macos"))] {
        radii(&mut p.corner_radii, t);
        p.blur_radius *= t.scale();
    }
}

fn primitive(p: &mut Primitive, t: PaintVectorTransform, m: TransformationMatrix) {
    match p {
        Primitive::Quad(p) => {
            p.transformation = m.compose(p.transformation);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
        },
        Primitive::MonochromeSprite(p) => {
            p.transformation = m.compose(p.transformation);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
        },
        Primitive::SubpixelSprite(p) => {
            p.transformation = m.compose(p.transformation);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
        },
        Primitive::PolychromeSprite(p) => {
            p.transformation = m.compose(p.transformation);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
        },
        Primitive::Shadow(p) => {
            p.bounds = map_bounds(p.bounds, m);
            p.element_bounds = map_bounds(p.element_bounds, m);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
            radii(&mut p.corner_radii, t);
            radii(&mut p.element_corner_radii, t);
            p.blur_radius = p.blur_radius * t.scale();
        },
        Primitive::Underline(p) => {
            p.bounds = map_bounds(p.bounds, m);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
            p.thickness = p.thickness * t.scale();
            p.pad = (p.pad + u32::from(t.quarter_turns())) % 4;
        },
        Primitive::Surface(p) => surface(p, t, m),
        Primitive::Path(p) => {
            let (bounds, inverse) = p.color_mapping.unwrap_or((p.clipped_bounds(), TransformationMatrix::unit()));
            p.color_mapping = Some((bounds, inverse.compose(t.inverse_of(m))));
            p.bounds = map_bounds(p.bounds, m);
            p.start = map_point(p.start, m);
            p.current = map_point(p.current, m);
            p.content_mask.bounds = map_bounds(p.content_mask.bounds, m);
            for v in &mut p.vertices {
                v.xy_position = map_point(v.xy_position, m);
                v.content_mask.bounds = map_bounds(v.content_mask.bounds, m);
            }
        },
    }
}

fn conjugate(h: [f32; 9], m: TransformationMatrix, inverse: TransformationMatrix) -> [f32; 9] {
    let expand = |m: TransformationMatrix| [m.rotation_scale[0][0], m.rotation_scale[0][1], m.translation[0],
        m.rotation_scale[1][0], m.rotation_scale[1][1], m.translation[1], 0., 0., 1.];
    let multiply = |a: [f32; 9], b: [f32; 9]| {
        std::array::from_fn(|i| (0..3).map(|k| a[i / 3 * 3 + k] * b[k * 3 + i % 3]).sum())
    };
    multiply(multiply(expand(m), h), expand(inverse))
}
