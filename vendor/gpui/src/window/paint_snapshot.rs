//! Снимок окружения и единый проверяемый replay с восстановлением через guard.

use super::*;

mod raster_env;
pub use raster_env::{RasterCaptures, RasterPaintCtx};

/// General element environment; raster placement is inherited from the caller.
#[doc = include_str!("paint_contract.md")]
#[derive(Clone)]
pub struct PaintCtx {
    identity: paint_scope::Identity,
    layout_owner: RetainedLayout,
    element_id_stack: SmallVec<[ElementId; 32]>,
    text_style_stack: TextStyleScope,
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
    /// KaminIDE patch: ink sinks open at capture keep measuring deferred paint.
    ink: Vec<Rc<InkSink>>,
    ink_scope: u32,
    /// KaminIDE patch: deferred paint keeps the line-layout release scope it
    /// was captured in (`Window::with_line_layouts_released`).
    layouts_released: bool,
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
        mem::swap(&mut window.next_frame.scene.ink, &mut self.ink);
        mem::swap(&mut window.next_frame.scene.ink_scope, &mut self.ink_scope);
        crate::text_system::swap_layouts_released(&mut self.layouts_released);
    }
}

impl Window {
    /// Capture the general environment used by positioned/prepaint consumers.
    pub fn paint_ctx(&self) -> PaintCtx {
        PaintCtx {
            identity: paint_scope::Identity::capture(self),
            layout_owner: self.retained_layout(),
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
            ink: self.next_frame.scene.ink.clone(),
            ink_scope: self.next_frame.scene.ink_scope,
            layouts_released: crate::text_system::layouts_released(),
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


    fn replay_paint<R>(
        &mut self,
        state: PaintState,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        state.base.identity.check(self)?;
        let mut layout_owner = state.base.layout_owner.clone();
        // Keep retained vectors outside recursive paint frames.
        let mut state = Box::new(state);
        let _contexts = self.render_contexts.enter_snapshot(mem::take(&mut state.base.render_contexts));
        state.restore(self);
        // KaminIDE patch: effects of deferred paint taint the sinks it feeds.
        let f = |window: &mut Self| {
            let probe = super::retained::InkProbe::open(window);
            let result = f(window);
            if let Some(probe) = probe {
                probe.close(window);
            }
            result
        };
        Ok(self.with_retained_layout(&mut layout_owner, |window| {
            window.with_restore(state, |window, state| state.restore(window), f).0
        }))
    }

    /// KaminIDE patch: `replay_paint` of a checked state kept by the caller (on the heap,
    /// outside recursive paint frames); the state holds the captured buffers again afterwards.
    fn replay_in<R>(&mut self, state: &mut PaintState, f: impl FnOnce(&mut Self) -> R) -> R {
        let mut layout_owner = state.base.layout_owner.clone();
        let contexts = self.render_contexts.enter_snapshot(mem::take(&mut state.base.render_contexts));
        state.restore(self);
        let f = |window: &mut Self| {
            let probe = super::retained::InkProbe::open(window);
            let result = f(window);
            if let Some(probe) = probe {
                probe.close(window);
            }
            result
        };
        let result = self.with_retained_layout(&mut layout_owner, |window| {
            window.with_restore(&mut *state, |window, state| state.restore(window), f).0
        });
        state.base.render_contexts = contexts.finish();
        result
    }
}
