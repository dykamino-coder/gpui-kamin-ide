//! Снимок окружения и единый проверяемый replay с восстановлением через guard.

use super::*;

/// General element environment; raster placement is inherited from the caller.
#[doc = include_str!("paint_contract.md")]
#[derive(Clone)]
pub struct PaintCtx {
    identity: paint_scope::Identity,
    element_id_stack: SmallVec<[ElementId; 32]>,
    text_style_stack: Vec<TextStyleRefinement>,
    render_contexts: render_context::Snapshot,
    rendered_entity_stack: Vec<EntityId>,
    element_offset_stack: Vec<Point<Pixels>>,
    exact_element_offset_depth: u32,
    transformation_stack: Vec<TransformationMatrix>,
    mask_scale: Option<(Point<Pixels>, f32)>,
    mask_map: Option<TransformationMatrix>,
    element_opacity: f32,
    content_mask_stack: Vec<ContentMask<Pixels>>,
    image_cache_stack: Vec<AnyImageCache>,
}

/// Full paint environment, including device scale and exact raster placement.
/// Cloning retains values and host Rc identities, but does not keep scopes alive.
#[derive(Clone)]
pub struct PaintState {
    base: PaintCtx,
    raster: RasterFrame,
    scale_factor: f32,
    rem_size_overrides: SmallVec<[Pixels; 8]>,
}

/// Paint environment retaining raster placement at the current device scale.
pub struct RasterPaintCtx {
    base: PaintCtx,
    raster: RasterFrame,
}

impl RasterPaintCtx {
    /// Select fill placement independently of the retained glyph placement.
    pub fn set_fill_transform(&mut self, transform: Option<(TransformationMatrix, TransformationMatrix)>) {
        self.raster.fill_transform = transform;
    }
}

impl PaintState {
    pub(super) fn restore(&mut self, window: &mut Window) {
        self.base.swap(window);
        mem::swap(&mut self.raster, &mut window.raster_frame);
        mem::swap(&mut window.scale_factor, &mut self.scale_factor);
        mem::swap(&mut window.rem_size_override_stack, &mut self.rem_size_overrides);
    }
}

impl PaintCtx {
    fn swap(&mut self, window: &mut Window) {
        mem::swap(&mut window.element_id_stack, &mut self.element_id_stack);
        mem::swap(&mut window.text_style_stack, &mut self.text_style_stack);
        mem::swap(&mut window.rendered_entity_stack, &mut self.rendered_entity_stack);
        mem::swap(&mut window.element_offset_stack, &mut self.element_offset_stack);
        mem::swap(&mut window.exact_element_offset_depth, &mut self.exact_element_offset_depth);
        mem::swap(&mut window.transformation_stack, &mut self.transformation_stack);
        mem::swap(&mut window.mask_scale, &mut self.mask_scale);
        mem::swap(&mut window.mask_map, &mut self.mask_map);
        mem::swap(&mut window.element_opacity, &mut self.element_opacity);
        mem::swap(&mut window.content_mask_stack, &mut self.content_mask_stack);
        mem::swap(&mut window.image_cache_stack, &mut self.image_cache_stack);
    }
}

impl Window {
    /// Capture the general environment used by positioned/prepaint consumers.
    pub fn paint_ctx(&self) -> PaintCtx {
        PaintCtx {
            identity: paint_scope::Identity::capture(self),
            element_id_stack: self.element_id_stack.clone(),
            text_style_stack: self.text_style_stack.clone(),
            render_contexts: self.render_contexts.snapshot(),
            rendered_entity_stack: self.rendered_entity_stack.clone(),
            element_offset_stack: self.element_offset_stack.clone(),
            exact_element_offset_depth: self.exact_element_offset_depth,
            transformation_stack: self.transformation_stack.clone(),
            mask_scale: self.mask_scale,
            mask_map: self.mask_map,
            element_opacity: self.element_opacity,
            content_mask_stack: self.content_mask_stack.clone(),
            image_cache_stack: self.image_cache_stack.clone(),
        }
    }

    /// Capture all paint and raster state in the current live composition scope.
    pub fn snapshot_paint_state(&self) -> PaintState {
        self.paint_state(self.paint_ctx(), self.raster_frame().clone())
    }

    fn paint_state(&self, base: PaintCtx, raster: RasterFrame) -> PaintState {
        PaintState {
            base,
            raster,
            scale_factor: self.scale_factor,
            rem_size_overrides: self.rem_size_override_stack.clone(),
        }
    }

    /// Replay a full snapshot. Invalid identity never invokes the callback.
    pub fn with_paint_state<R>(
        &mut self,
        state: PaintState,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint(state, f)
    }

    /// Replay the general environment, inheriting the caller's raster placement.
    pub fn with_paint_ctx<R>(
        &mut self,
        ctx: PaintCtx,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint(self.paint_state(ctx, self.raster_frame().clone()), f)
    }

    /// Capture environment and raster placement without a scale override.
    pub fn raster_paint_ctx(&self) -> RasterPaintCtx {
        RasterPaintCtx { base: self.paint_ctx(), raster: self.raster_frame().clone() }
    }

    /// Replay captured raster placement in the original live scope.
    pub fn with_raster_paint_ctx<R>(
        &mut self,
        ctx: RasterPaintCtx,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        self.replay_paint(self.paint_state(ctx.base, ctx.raster), f)
    }

    fn replay_paint<R>(
        &mut self,
        state: PaintState,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        state.base.identity.check(self)?;
        // Keep retained vectors outside recursive paint frames.
        let mut state = Box::new(state);
        let _contexts = self.render_contexts.enter_snapshot(mem::take(&mut state.base.render_contexts));
        state.restore(self);
        Ok(self.with_restore(state, |window, state| state.restore(window), f).0)
    }
}
