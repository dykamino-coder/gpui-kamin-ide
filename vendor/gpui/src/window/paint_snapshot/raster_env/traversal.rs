//! KaminIDE patch: replay shared raster buffers across suspended child paint.

use super::*;

impl Window {
    /// Replay captured raster placement until the child continuation completes.
    pub async fn with_raster_paint_ctx_async<R>(
        &mut self,
        ctx: RasterPaintCtx,
        captures: &RasterCaptures,
        f: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        let RasterPaintCtx {
            env,
            ids,
            glyph_offset,
            fill_transform,
            exact_bounds,
        } = ctx;
        env.base.identity.check(self)?;
        let (base, contexts, mut buffers) = match captures.release(env) {
            Ok(RasterEnv { mut base, contexts }) => {
                base.element_id_stack.extend(ids);
                (base, contexts, None)
            }
            Err(env) => {
                let mut buffers = captures.buffers.take().unwrap_or_default();
                let base = buffers.fill(&env, &ids);
                let contexts = mem::take(&mut buffers.raster_contexts);
                (base, contexts, Some(buffers))
            }
        };
        let mut state = PaintState {
            base,
            raster: RasterFrame {
                glyph_offset,
                fill_transform,
                exact_bounds,
                contexts,
            },
            scale_factor: self.scale_factor,
            rem_size_overrides: self.rem_size_override_stack.clone(),
        };
        let result = self.replay_in_async(&mut state, f).await;
        if let Some(buffers) = buffers.as_mut() {
            buffers.reclaim(state);
        }
        if let Some(buffers) = buffers {
            captures.buffers.set(Some(buffers));
        }
        Ok(result)
    }
}
