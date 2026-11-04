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
    /// Восстанавливает окружение и raster placement отложенного задания.
    pub async fn with_raster_paint_ctx_async<R>(
        &mut self,
        ctx: RasterPaintCtx,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint_async(self.paint_state(ctx.base, ctx.raster), f)
            .await
    }
    pub(super) async fn replay_paint_async<R>(
        &mut self,
        state: PaintState,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        state.base.identity.check(self)?;
        let mut layout_owner = state.base.layout_owner.clone();
        // Keep retained vectors outside recursive paint frames.
        let mut state = Box::new(state);
        let _contexts = self
            .render_contexts
            .enter_snapshot(mem::take(&mut state.base.render_contexts));
        state.restore(self);
        // KaminIDE patch: effects of deferred paint taint the sinks it feeds.
        let f = async |window: &mut Self| {
            let probe = crate::window::retained::InkProbe::open(window);
            let result = f(window).await;
            if let Some(probe) = probe {
                probe.close(window);
            }
            result
        };
        Ok(self
            .with_retained_layout_async(&mut layout_owner, async |window| {
                window
                    .with_restore_async(state, |window, state| state.restore(window), f)
                    .await
                    .0
            })
            .await)
    }
}
