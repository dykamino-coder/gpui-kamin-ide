//! Запись один раз и атомарный вызов плана в свежей композиционной области.

use super::*;
mod traversal;

impl Window {
    /// Consume source paint once, replacing the ambient fragment clip explicitly.
    /// Layout/prepaint/event registration remain the caller's one-time work.
    pub fn record_paint<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        paint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<(R, PaintRecord), PaintRecordError> {
        reblessive::Stack::new()
            .enter(|stack| {
                self.record_paint_async(source_clip, cx, stack, async |window, cx, _| {
                    paint(window, cx)
                })
            })
            .finish()
    }

    pub(super) fn capture_paint_commands<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        paint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<(R, Scene, paint_scope::Identity, f32), PaintRecordError> {
        reblessive::Stack::new()
            .enter(|stack| {
                self.capture_paint_commands_async(source_clip, cx, stack, async |window, cx, _| {
                    paint(window, cx)
                })
            })
            .finish()
    }

    /// Invoke immutable commands under new scopes, without any element callback.
    /// The returned closed scope identifies this invocation; it grants no replay right.
    pub fn paint_plan(
        &mut self,
        plan: &PaintPlan,
        placement: PaintPlacement,
    ) -> Result<PaintScopeId, PaintRecordError> {
        let presentation = self.paint_presentation();
        let extent = plan.validate(self, presentation)?;
        paint_scope::Identity::capture(self).check_pass(self)?;
        if self.invalidator.inner.borrow().draw_phase != DrawPhase::Paint {
            return Err(PaintRecordError::NotPainting);
        }
        let delta = placement.destination - placement.source;
        let scaled = delta.scale(self.scale_factor());
        if !validation::point_valid(placement.source)
            || !validation::point_valid(placement.destination)
            || !validation::point_valid(delta)
            || !validation::scaled_bounds_valid(placement.clip, self.scale_factor())
            || !validation::scaled_bounds_valid(
                Bounds::new(extent.origin + delta, extent.size),
                self.scale_factor(),
            )
            || !scaled.x.0.is_finite()
            || !scaled.y.0.is_finite()
        {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let clip = placement
            .clip
            .intersect(&self.content_mask().bounds)
            .scale(self.scale_factor());
        let (invocation, scene) = {
            // Неизменяемый план — один лист сегментного обхода;
            // его собственные эффекты разрешены до привязки к сегментам.
            let _recording = self.render_contexts.enter::<segments::Recording>(None);
            self.record_scene(extent, true, |window| {
                let id = window.paint_scope_id();
                plan.paint(window, presentation);
                id
            })
        };
        // No destination writes occur until the complete invocation has succeeded.
        paint_scope::Identity::capture(self).check_pass(self)?;
        if !self
            .next_frame
            .scene
            .replay_placed(&scene, scaled, Some(clip))
        {
            let scene = scene.copy_record(scaled, Some(clip));
            let scene = self.adopt_scene(scene);
            self.next_frame.scene.replay_record(&scene);
        }
        Ok(invocation)
    }

    fn record_scene<R>(
        &mut self,
        clip: Bounds<Pixels>,
        explicit_coordinates: bool,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> (R, Scene) {
        reblessive::Stack::new()
            .enter(|_| {
                self.record_scene_async(clip, explicit_coordinates, async |window| paint(window))
            })
            .finish()
    }
}
