//! Replay окружения до завершения продолжения потомка.

use super::*;

impl Window {
    /// Восстанавливает полное окружение для продолжения paint.
    pub async fn with_paint_state_async<R>(
        &mut self,
        state: PaintState,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint_async(state, f).await
    }
    /// Восстанавливает окружение с raster placement вызывающей стороны.
    pub async fn with_paint_ctx_async<R>(
        &mut self,
        ctx: PaintCtx,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint_async(self.paint_state(ctx, self.raster_frame().clone()), f)
            .await
    }
    pub(super) async fn replay_paint_async<R>(
        &mut self,
        state: PaintState,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        state.base.identity.check(self)?;
        // Keep retained vectors outside recursive paint frames.
        let mut state = Box::new(state);
        Ok(self.replay_in_async(&mut state, f).await)
    }

    // The caller owns the captured buffers and may reclaim them after replay.
    pub(super) async fn replay_in_async<R>(
        &mut self,
        state: &mut PaintState,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> R {
        let mut layout_owner = state.base.layout_owner.clone();
        let contexts = self
            .render_contexts
            .enter_snapshot(mem::take(&mut state.base.render_contexts));
        state.restore(self);
        let f = async |window: &mut Self| {
            let probe = crate::window::retained::InkProbe::open(window);
            let result = f(window).await;
            if let Some(probe) = probe {
                probe.close(window);
            }
            result
        };
        let result = self
            .with_retained_layout_async(&mut layout_owner, async |window| {
                window
                    .with_restore_async(&mut *state, |window, state| state.restore(window), f)
                    .await
                    .0
            })
            .await;
        state.base.render_contexts = contexts.finish();
        result
    }
}
