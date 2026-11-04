//! Запись живёт до завершения потомков и отложенных команд хозяина.

use super::*;

impl Window {
    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn record_paint_async<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        paint: impl AsyncFnOnce(&mut Self, &mut App, &mut crate::PaintStack) -> R,
    ) -> Result<(R, PaintRecord), PaintRecordError> {
        let _recording = self.render_contexts.enter::<segments::Recording>(None);
        let (result, scene, identity, scale) = self
            .capture_paint_commands_async(source_clip, cx, stack, paint)
            .await?;
        Ok((
            result,
            PaintRecord(Rc::new(Recorded {
                identity,
                scale,
                clip: source_clip,
                scene,
            })),
        ))
    }

    pub(in crate::window::paint_record) async fn capture_paint_commands_async<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        paint: impl AsyncFnOnce(&mut Self, &mut App, &mut crate::PaintStack) -> R,
    ) -> Result<(R, Scene, paint_scope::Identity, f32), PaintRecordError> {
        let identity = paint_scope::Identity::capture(self);
        identity.check_pass(self)?;
        if !validation::scaled_bounds_valid(source_clip, self.scale_factor())
            || !self.scale_factor().is_finite()
            || self.scale_factor() <= 0.0
        {
            return Err(PaintRecordError::InvalidGeometry);
        }
        if self.invalidator.inner.borrow().draw_phase != DrawPhase::Paint {
            return Err(PaintRecordError::NotPainting);
        }
        let scale = self.scale_factor();
        let (result, mut scene) = self
            .record_scene_async(source_clip, false, async |window| {
                let recording = window
                    .render_context::<crate::PaintTraversal>()
                    .and_then(|traversal| traversal.recording.clone());
                let mut guard = recording
                    .as_ref()
                    .map(|recording| (recording.begin)(window));
                let result = paint(window, cx, stack).await;
                if let (Some(recording), Some(guard)) = (recording, guard.as_mut()) {
                    (recording.finish)(guard.as_mut(), window, cx, stack).await;
                }
                result
            })
            .await;
        identity.check_pass(self)?;
        scene.finish_frozen();
        Ok((result, scene, identity, scale))
    }

    pub(super) async fn record_scene_async<R>(
        &mut self,
        clip: Bounds<Pixels>,
        explicit_coordinates: bool,
        paint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> (R, Scene) {
        let state = Box::new(self.snapshot_paint_state());
        let contexts = self.render_contexts.snapshot();
        let _contexts = self.render_contexts.enter_snapshot(contexts);
        let planes = mem::take(&mut self.plane_contexts);
        self.with_restore_async(
            (state, planes),
            |window, state| {
                state.0.restore(window);
                mem::swap(&mut window.plane_contexts, &mut state.1);
            },
            async |window| {
                // Source/destination clips are already in explicit window coordinates.
                window.content_mask_stack = vec![ContentMask { bounds: clip }];
                if explicit_coordinates {
                    window.transformation_stack.clear();
                    window.mask_map = None;
                    window.mask_scale = None;
                    window.element_opacity = 1.0;
                }
                window.with_isolated_scene_async(paint).await
            },
        )
        .await
        .0
    }
}
