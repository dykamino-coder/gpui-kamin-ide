//! Копирование неизменяемых команд с явным переносом и выходным clip.

use super::*;

mod geometry;

impl Scene {
    pub(crate) fn copy_record(
        &self,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> Self {
        let mut scene = self.copy_commands(offset, clip);
        // The compositor keeps dependencies flat, children before parents.
        scene.groups = self.groups.iter().map(|group| PaintGroup {
            scene: group.scene.copy_commands(offset, None),
            bounds: geometry::bounds(group.bounds, offset),
            blur_radius: group.blur_radius,
            blend: group.blend,
            polygon: group.polygon.iter().map(|p| *p + offset).collect(),
            polygon_inverse: group.polygon_inverse,
            mask: group.mask.clone(),
            mask_bounds: geometry::bounds(group.mask_bounds, offset),
            mask_once: group.mask_once,
            mask_clip: group.mask_clip.map(|mut b| {
                b[0] += offset.x.0;
                b[1] += offset.y.0;
                b
            }),
            projection: group.projection.map(|m| geometry::projection(m, offset)),
            fragments: group.fragments.iter().map(|fragment| {
                fragment.iter().map(|p| *p + offset).collect()
            }).collect(),
        }).collect();
        scene
    }

    fn copy_commands(
        &self,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> Self {
        let mut scene = Self { in_group: self.in_group, ..Self::default() };
        for command in &self.paint_operations {
            let mut command = command.clone();
            match &mut command {
                PaintOperation::Primitive(primitive) => geometry::primitive(primitive, offset, clip),
                PaintOperation::StartLayer(b) => *b = geometry::bounds(*b, offset),
                _ => {},
            }
            scene.replay_operation(command);
        }
        scene.hoisted_backdrops = self.hoisted_backdrops.iter().map(|surface| {
            let mut surface = surface.clone();
            surface.bounds = geometry::bounds(surface.bounds, offset);
            geometry::mask(&mut surface.content_mask, offset, clip);
            surface
        }).collect();
        scene.finish();
        scene
    }
}
