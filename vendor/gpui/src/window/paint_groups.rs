//! Композит группы после завершения продолжения её поддерева.

use super::*;

impl Window {
    /// Рисует группу, сохраняя изолированную сцену на явном стеке обхода.
    #[allow(clippy::too_many_arguments)]
    pub async fn paint_group_async<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        blur_radius: f32,
        opacity: f32,
        blend: u32,
        polygon: &[Point<Pixels>],
        mask: Option<(std::sync::Arc<crate::RenderImage>, Bounds<Pixels>, u32)>,
        mask_clip: Option<[f32; 4]>,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        use crate::{PaintGroup, PaintSurface};

        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let scaled = bounds.scale(scale_factor);
        let content_mask = self.content_mask().scale(scale_factor);

        let (result, mut inner) = self.with_isolated_scene_async(f).await;
        inner.finish();
        // KaminIDE patch: подложки, поднятые из группы (`paint_backdrop_filter`
        // с `hoist`), — в объемлющую сцену; до кадра — сквозь все уровни.
        let hoisted = inner.take_backdrops();
        // Группы, вложенные в эту, переезжают в общий список кадра ПЕРЕД ней:
        // их номера были местными, поэтому сдвигаются на длину списка.
        let mut nested = std::mem::take(&mut inner.groups);
        let base = self.next_frame.scene.groups.len() as u32;
        if !nested.is_empty() {
            inner.shift_group_ids(base);
            for group in &mut nested {
                group.scene.shift_group_ids(base);
            }
            self.next_frame.scene.groups.append(&mut nested);
        }
        let index = self.next_frame.scene.groups.len() as u32;
        let (mask, mask_bounds, mask_once) = match mask {
            Some((img, b, once)) => (Some(img), b.scale(scale_factor), once),
            None => (None, Bounds::default(), 0),
        };
        self.next_frame.scene.groups.push(PaintGroup {
            scene: inner,
            bounds: scaled,
            blur_radius: blur_radius * scale_factor,
            blend,
            polygon: polygon.iter().map(|p| p.scale(scale_factor)).collect(),
            polygon_inverse: false,
            mask,
            mask_bounds,
            mask_once,
            mask_clip,
            projection: None,
            fragments: Vec::new(),
        });
        // Поднятые подложки ложатся ПЕРЕД меткой группы: проход подложки
        // копирует кадр, уже нарисованный под элементом, а буфер группы
        // (фон, рамка, дети — filter-effects-2 §3 шаг 5) композитится поверх.
        self.next_frame.scene.adopt_backdrops(hoisted);
        let unbounded = self.content_mask_stack.len() == 1;
        self.next_frame.scene.insert_group_surface(
            PaintSurface {
                order: 0,
                bounds: scaled,
                content_mask,
                corner_radii: corner_radii.scale(scale_factor),
                blur_radius: 0.0,
                group: index + 1,
                opacity,
                color_matrix: None,
                fragment: 0,
            },
            unbounded,
        );
        result
    }
}
