//! Place planes without changing composite geometry or paint order.

use super::*;

impl Window {
    /// Move a finished plane scene into the frame's group list (nested
    /// groups first, as `paint_group` does); returns its group index.
    pub(super) fn push_plane_group(
        &mut self,
        mut inner: Scene,
        projection: Option<[f32; 9]>,
        polygon: Vec<Point<ScaledPixels>>,
        fragments: Vec<Vec<Point<ScaledPixels>>>,
    ) -> u32 {
        let hoisted = inner.take_backdrops();
        let mut nested = std::mem::take(&mut inner.groups);
        let base = self.next_frame.scene.groups.len() as u32;
        if !nested.is_empty() {
            inner.shift_group_ids(base);
            for group in &mut nested {
                group.scene_mut().shift_group_ids(base);
            }
            self.next_frame.scene.groups.append(&mut nested);
        }
        let index = self.next_frame.scene.groups.len() as u32;
        let size = self.viewport_size.scale(self.scale_factor());
        let bounds = coverage::source(&inner, Bounds::new(Point::default(), size));
        self.next_frame.scene.groups.push(PaintGroup {
            scene: std::rc::Rc::new(inner),
            bounds,
            polygon,
            projection,
            fragments,
            ..Default::default()
        });
        self.next_frame.scene.adopt_backdrops(hoisted);
        index
    }

    pub(super) fn insert_plane_mark(
        &mut self,
        index: u32,
        fragment: u32,
        mask: ContentMask<ScaledPixels>,
    ) {
        self.next_frame.scene.insert_group_surface(
            PaintSurface {
                order: 0,
                bounds: mask.bounds,
                content_mask: mask,
                corner_radii: Corners::default(),
                blur_radius: 0.0,
                group: index + 1,
                opacity: 1.0,
                color_matrix: None,
                fragment,
            },
            self.content_mask_stack.len() == 1,
        );
    }
}
