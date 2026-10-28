//! Запись один раз и атомарный вызов плана в свежей композиционной области.

use super::*;

impl Window {
    /// Consume source paint once, replacing the ambient fragment clip explicitly.
    /// Layout/prepaint/event registration remain the caller's one-time work.
    pub fn record_paint<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        paint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<(R, PaintRecord), PaintRecordError> {
        let _recording = self.render_contexts.enter::<segments::Recording>(None);
        let (result, scene, identity, scale) = self.capture_paint_commands(source_clip, cx, paint)?;
        Ok((result, PaintRecord(Rc::new(Recorded { identity, scale, clip: source_clip, scene }))))
    }

    pub(super) fn capture_paint_commands<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        paint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<(R, Scene, paint_scope::Identity, f32), PaintRecordError> {
        let identity = paint_scope::Identity::capture(self);
        identity.check_pass(self)?;
        if !validation::scaled_bounds_valid(source_clip, self.scale_factor()) || !self.scale_factor().is_finite()
            || self.scale_factor() <= 0.0 {
            return Err(PaintRecordError::InvalidGeometry);
        }
        if self.invalidator.inner.borrow().draw_phase != DrawPhase::Paint {
            return Err(PaintRecordError::NotPainting);
        }
        let scale = self.scale_factor();
        let (result, mut scene) = self.record_scene(source_clip, false, |window| {
            let recording = window.render_context::<crate::PaintTraversal>()
                .and_then(|traversal| traversal.recording.clone());
            let mut guard = recording.as_ref().map(|recording| (recording.begin)(window));
            let result = paint(window, cx);
            if let (Some(recording), Some(guard)) = (recording, guard.as_mut()) {
                (recording.finish)(guard.as_mut(), window, cx);
            }
            result
        });
        identity.check_pass(self)?;
        scene.finish();
        Ok((result, scene, identity, scale))
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
            || !validation::point_valid(delta) || !validation::scaled_bounds_valid(placement.clip, self.scale_factor())
            || !validation::scaled_bounds_valid(Bounds::new(extent.origin + delta, extent.size), self.scale_factor())
            || !scaled.x.0.is_finite() || !scaled.y.0.is_finite() {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let clip = placement.clip.intersect(&self.content_mask().bounds).scale(self.scale_factor());
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
        let scene = scene.copy_record(scaled, Some(clip));
        let scene = self.adopt_scene(scene);
        self.next_frame.scene.replay(0..scene.paint_operations.len(), &scene);
        Ok(invocation)
    }

    fn record_scene<R>(
        &mut self,
        clip: Bounds<Pixels>,
        explicit_coordinates: bool,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> (R, Scene) {
        let state = Box::new(self.snapshot_paint_state());
        let contexts = self.render_contexts.snapshot();
        let _contexts = self.render_contexts.enter_snapshot(contexts);
        let planes = mem::take(&mut self.plane_contexts);
        self.with_restore((state, planes), |window, state| {
            state.0.restore(window);
            mem::swap(&mut window.plane_contexts, &mut state.1);
        }, |window| {
            // Source/destination clips are already in explicit window coordinates.
            window.content_mask_stack = vec![ContentMask { bounds: clip }];
            if explicit_coordinates {
                window.transformation_stack.clear();
                window.mask_map = None;
                window.mask_scale = None;
                window.element_opacity = 1.0;
            }
            window.with_isolated_scene(paint)
        }).0
    }
}
