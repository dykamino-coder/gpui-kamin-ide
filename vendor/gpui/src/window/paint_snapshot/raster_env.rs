//! KaminIDE patch: raster captures of sibling elements share one environment.
//!
//! A deferred inline job used to carry a full copy of the paint environment (~1.5 KB, the
//! element id stack inline, a clone of every stack and of both context tables): thousands
//! of jobs per frame cost several allocations each and a queue of megabytes. As Blink's
//! paint chunks point at shared property tree nodes instead of copying transforms and clips
//! (`PropertyTreeState`), a capture here keeps a reference to the environment of its parent
//! (the window remembers the last one and reuses it while it still matches) plus what differs
//! per element: its own element id, glyph offset, fill transform and exact bounds. Replay
//! fills the window from the shared environment into reused buffers, so neither capture nor
//! replay allocates while the environment holds.

use super::*;

/// The environment a raster capture shares with its siblings. Its element id stack is the
/// prefix common to them (the parent's ids).
pub(in crate::window) struct RasterEnv {
    base: PaintCtx,
    contexts: render_context::Snapshot,
}

/// Paint environment retaining raster placement at the current device scale.
pub struct RasterPaintCtx {
    env: Rc<RasterEnv>,
    /// Element ids below the shared prefix (the capturing element's own).
    ids: SmallVec<[ElementId; 1]>,
    glyph_offset: Point<Pixels>,
    fill_transform: Option<(TransformationMatrix, TransformationMatrix)>,
    exact_bounds: Option<(Bounds<Pixels>, Bounds<Pixels>)>,
}

impl RasterPaintCtx {
    /// Select fill placement independently of the retained glyph placement.
    pub fn set_fill_transform(&mut self, transform: Option<(TransformationMatrix, TransformationMatrix)>) {
        self.fill_transform = transform;
    }

    /// Whether `self` and `other` share one captured environment (no copy of their own).
    pub fn shares_environment(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.env, &other.env)
    }
}

impl RasterEnv {
    fn capture(window: &Window, prefix: usize) -> Self {
        let mut base = window.paint_ctx();
        base.element_id_stack.truncate(prefix);
        Self { base, contexts: window.raster_frame.contexts.clone() }
    }

    /// Everything but the capturing element's own id and placement equals the window's.
    fn matches(&self, window: &Window) -> bool {
        let b = &self.base;
        let prefix = b.element_id_stack.len();
        let depth = window.element_id_stack.len();
        (depth == prefix || depth == prefix + 1)
            && b.identity.is_current(window)
            && b.layout_owner.same(&window.layout_owner)
            && window.element_id_stack[..prefix] == b.element_id_stack[..]
            && b.text_style_stack.same(&window.text_style_stack)
            && window.render_contexts.matches(&b.render_contexts)
            && b.rendered_entity_stack == window.rendered_entity_stack
            && b.element_offset_stack == window.element_offset_stack
            && b.exact_element_offset_depth == window.exact_element_offset_depth
            && b.transformation_stack == window.transformation_stack
            && b.mask_scale == window.mask_scale
            && b.mask_map == window.mask_map
            && b.element_opacity.to_bits() == window.element_opacity.to_bits()
            && b.content_mask_stack == window.content_mask_stack
            && b.image_cache_stack.len() == window.image_cache_stack.len()
            && b.image_cache_stack.iter().zip(&window.image_cache_stack).all(|(a, b)| a.same(b))
            && b.ink.len() == window.next_frame.scene.ink.len()
            && b.ink.iter().zip(&window.next_frame.scene.ink).all(|(a, b)| Rc::ptr_eq(a, b))
            && self.contexts == window.raster_frame.contexts
    }
}

/// One raster replay: its state on the heap (outside recursive paint frames) and, between
/// replays, the empty buffers of that state. Kept for the replays of a frame.
#[derive(Default)]
pub(in crate::window) struct ReplaySlot {
    state: Option<PaintState>,
    buffers: ReplayBuffers,
}

#[derive(Default)]
struct ReplayBuffers {
    ids: SmallVec<[ElementId; 32]>,
    contexts: render_context::Snapshot,
    rendered: Vec<EntityId>,
    offsets: Vec<Point<Pixels>>,
    transforms: Vec<TransformationMatrix>,
    masks: Vec<ContentMask<Pixels>>,
    images: Vec<AnyImageCache>,
    ink: Vec<Rc<InkSink>>,
    raster_contexts: render_context::Snapshot,
    rem: SmallVec<[Pixels; 8]>,
}

impl ReplayBuffers {
    /// Take the buffers back from a finished replay, empty, keeping their capacity.
    fn reclaim(&mut self, state: PaintState) {
        let PaintState { base, raster, rem_size_overrides, .. } = state;
        self.ids = base.element_id_stack;
        self.contexts = base.render_contexts;
        self.rendered = base.rendered_entity_stack;
        self.offsets = base.element_offset_stack;
        self.transforms = base.transformation_stack;
        self.masks = base.content_mask_stack;
        self.images = base.image_cache_stack;
        self.ink = base.ink;
        self.raster_contexts = raster.contexts;
        self.rem = rem_size_overrides;
        self.ids.clear();
        self.contexts.clear();
        self.rendered.clear();
        self.offsets.clear();
        self.transforms.clear();
        self.masks.clear();
        self.images.clear();
        self.ink.clear();
        self.raster_contexts.clear();
        self.rem.clear();
    }
}

impl Window {
    /// Capture environment and raster placement without a scale override.
    pub fn raster_paint_ctx(&self) -> RasterPaintCtx {
        let env = match self.raster_env.take() {
            Some(env) if env.matches(self) => env,
            _ => Rc::new(RasterEnv::capture(self, self.element_id_stack.len().saturating_sub(1))),
        };
        self.raster_env.set(Some(env.clone()));
        let prefix = env.base.element_id_stack.len();
        RasterPaintCtx {
            ids: self.element_id_stack[prefix..].iter().cloned().collect(),
            glyph_offset: self.raster_frame.glyph_offset,
            fill_transform: self.raster_frame.fill_transform,
            exact_bounds: self.raster_frame.exact_bounds,
            env,
        }
    }

    /// Replay captured raster placement in the original live scope.
    pub fn with_raster_paint_ctx<R>(
        &mut self,
        ctx: RasterPaintCtx,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        let env = &ctx.env;
        let b = &env.base;
        b.identity.check(self)?;
        let mut slot: Box<ReplaySlot> = self.raster_replay_spares.pop().unwrap_or_default();
        let buffers = &mut slot.buffers;
        buffers.ids.extend(b.element_id_stack.iter().chain(&ctx.ids).cloned());
        b.render_contexts.clone_into(&mut buffers.contexts);
        buffers.rendered.extend_from_slice(&b.rendered_entity_stack);
        buffers.offsets.extend_from_slice(&b.element_offset_stack);
        buffers.transforms.extend_from_slice(&b.transformation_stack);
        buffers.masks.extend_from_slice(&b.content_mask_stack);
        buffers.images.extend(b.image_cache_stack.iter().cloned());
        buffers.ink.extend(b.ink.iter().cloned());
        env.contexts.clone_into(&mut buffers.raster_contexts);
        buffers.rem.extend_from_slice(&self.rem_size_override_stack);
        slot.state = Some(PaintState {
            base: PaintCtx {
                identity: b.identity.clone(),
                layout_owner: b.layout_owner.clone(),
                element_id_stack: mem::take(&mut buffers.ids),
                text_style_stack: b.text_style_stack.clone(),
                render_contexts: mem::take(&mut buffers.contexts),
                rendered_entity_stack: mem::take(&mut buffers.rendered),
                element_offset_stack: mem::take(&mut buffers.offsets),
                exact_element_offset_depth: b.exact_element_offset_depth,
                transformation_stack: mem::take(&mut buffers.transforms),
                mask_scale: b.mask_scale,
                mask_map: b.mask_map,
                element_opacity: b.element_opacity,
                content_mask_stack: mem::take(&mut buffers.masks),
                image_cache_stack: mem::take(&mut buffers.images),
                ink: mem::take(&mut buffers.ink),
            },
            raster: RasterFrame {
                glyph_offset: ctx.glyph_offset,
                fill_transform: ctx.fill_transform,
                exact_bounds: ctx.exact_bounds,
                contexts: mem::take(&mut buffers.raster_contexts),
            },
            scale_factor: self.scale_factor,
            rem_size_overrides: mem::take(&mut buffers.rem),
        });
        drop(ctx);
        let state = slot.state.as_mut().expect("raster replay state");
        let result = self.replay_in(state, f);
        if let Some(state) = slot.state.take() {
            slot.buffers.reclaim(state);
        }
        self.raster_replay_spares.push(slot);
        Ok(result)
    }

    /// Forget the shared raster environment and the replay buffers once a frame is drawn,
    /// so that nothing they hold outlives it.
    pub(in crate::window) fn release_raster_env(&mut self) {
        self.raster_env.take();
        self.raster_replay_spares.clear();
    }
}
