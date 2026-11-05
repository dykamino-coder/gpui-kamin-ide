//! Продолжения композиции удерживают сцены до завершения дочернего обхода.

use super::*;

impl Window {
    /// Paint `f` as one 3D rendering context: planes opened inside it
    /// (`paint_plane`) are depth-sorted and split when it closes.
    pub async fn paint_plane_context_async<R>(&mut self, f: impl AsyncFnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();
        let content_mask = self.content_mask().scale(self.scale_factor());
        self.plane_contexts.push(PlaneContext {
            pending: Vec::new(),
            content_mask,
        });
        let (result, ctx) = self
            .with_restore_async(
                None,
                |window, ctx| {
                    *ctx = window.plane_contexts.pop();
                },
                async |window| window.with_paint_scope_async(f).await,
            )
            .await;
        let Some(ctx) = ctx else { return result };
        let records: Vec<PlaneRecord> = ctx.pending.into_iter().flatten().collect();
        let mut polys = Vec::new();
        for (i, rec) in records.iter().enumerate() {
            if let Some((pts, _)) = near_clip(&rec.corners) {
                let poly = PlanePoly::new(pts, i);
                // Edge-on planes are invisible and must not split others.
                if !poly.degenerate() {
                    polys.push(poly);
                }
            }
        }
        let ordered = back_to_front(polys);
        let mut pieces = vec![0usize; records.len()];
        for p in &ordered {
            pieces[p.owner] += 1;
        }
        let mut ids = vec![None; records.len()];
        let mut inline: Vec<Option<Scene>> = (0..records.len()).map(|_| None).collect();
        let mut first = vec![0u32; records.len()];
        for (i, rec) in records.into_iter().enumerate() {
            if pieces[i] == 0 {
                continue;
            }
            let polygon = match near_clip(&rec.corners) {
                Some((pts, true)) => to_points(&pts),
                _ => Vec::new(),
            };
            // An affine plane that is neither cut nor clipped needs no
            // buffer: its primitives are replayed straight into the frame at
            // its place in the BSP order (byte-identical to painting it
            // directly; a buffer would requantize glyph coverage).
            if rec.projection.is_none() && pieces[i] == 1 && polygon.is_empty() {
                inline[i] = Some(self.adopt_scene(rec.scene));
                continue;
            }
            let fragments = if pieces[i] > 1 {
                ordered
                    .iter()
                    .filter(|p| p.owner == i)
                    .map(|p| to_points(&grow_box_edges(p)))
                    .collect()
            } else {
                Vec::new()
            };
            ids[i] = Some(self.push_plane_group(rec.scene, rec.projection, polygon, fragments));
        }
        for p in &ordered {
            if let Some(scene) = inline[p.owner].take() {
                let ops = scene.paint_operations.len();
                self.next_frame.scene.replay(0..ops, &scene);
                continue;
            }
            let Some(id) = ids[p.owner] else { continue };
            let fragment = if pieces[p.owner] > 1 {
                first[p.owner] += 1;
                first[p.owner]
            } else {
                0
            };
            self.insert_plane_mark(id, fragment, ctx.content_mask);
        }
        result
    }

    /// Paint `f` as one plane (see the module docs). Inside an open context
    /// the plane is composited when the context closes; otherwise at once.
    pub async fn paint_plane_async<R>(
        &mut self,
        spec: PlaneSpec,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint();
        let slot = self.plane_contexts.last_mut().map(|c| {
            c.pending.push(None);
            c.pending.len() - 1
        });
        let depth = self.plane_contexts.len();
        let content_mask = self.content_mask().scale(self.scale_factor());
        let r = spec.raster.rotation_scale;
        let keeps_axes = (r[0][1].abs() < 1e-4 && r[1][0].abs() < 1e-4)
            || (r[0][0].abs() < 1e-4 && r[1][1].abs() < 1e-4);
        let map = (spec.masked && keeps_axes).then_some(spec.raster);
        let (result, mut scene) = self
            .with_isolated_scene_async(async |window| {
                let state = (
                    mem::replace(&mut window.transformation_stack, vec![spec.raster]),
                    mem::replace(&mut window.mask_map, map),
                    mem::take(&mut window.content_mask_stack),
                );
                window
                    .with_restore_async(
                        state,
                        |window, state| {
                            mem::swap(&mut window.transformation_stack, &mut state.0);
                            mem::swap(&mut window.mask_map, &mut state.1);
                            mem::swap(&mut window.content_mask_stack, &mut state.2);
                        },
                        f,
                    )
                    .await
                    .0
            })
            .await;
        scene.finish();
        let mut rect = spec.rect;
        if let Some((ix0, iy0, ix1, iy1)) = ink_bounds(&scene, spec.raster) {
            rect = (
                rect.0.min(ix0),
                rect.1.min(iy0),
                rect.2.max(ix1),
                rect.3.max(iy1),
            );
        }
        let record = PlaneRecord {
            scene,
            projection: spec.projection,
            corners: plane_corners(&spec.full, spec.outer, rect),
        };
        match slot {
            Some(i) if self.plane_contexts.len() == depth => {
                if let Some(ctx) = self.plane_contexts.last_mut() {
                    ctx.pending[i] = Some(record);
                }
            }
            _ => {
                let Some((pts, clipped)) = near_clip(&record.corners) else {
                    return result;
                };
                let polygon = if clipped { to_points(&pts) } else { Vec::new() };
                let id =
                    self.push_plane_group(record.scene, record.projection, polygon, Vec::new());
                self.insert_plane_mark(id, 0, content_mask);
            }
        }
        result
    }
}
