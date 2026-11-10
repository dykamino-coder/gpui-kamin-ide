//! KaminIDE patch: raster captures of sibling elements share one environment.
//!
//! A deferred inline job used to carry a full copy of the paint environment (~1.5 KB, the
//! element id stack inline, a clone of every stack and of both context tables): thousands
//! of jobs per frame cost several allocations each and a queue of megabytes. As Blink's
//! paint chunks point at shared property tree nodes instead of copying transforms and clips
//! (`PropertyTreeState`), a capture here keeps a reference to the environment of its parent
//! (the caller's pass remembers the last one and reuses it while it still matches) plus what
//! differs per element: its own element id, glyph offset, fill transform and exact bounds.
//! Replay fills the window from the shared environment into reused buffers, so neither capture
//! nor replay allocates while the environment holds. That scratch belongs to the caller's
//! paint pass (`RasterCaptures`), as Blink's paint scratch belongs to the `PaintController` of
//! one paint, not to the window: it lives no longer than the pass.

use super::*;

/// KaminIDE patch: scratch of the raster captures and replays of one paint pass: the last
/// shared environment and the buffers of a finished replay of a shared environment. Owned by
/// the caller's pass and cleared when the pass ends, so nothing of it outlives the paint.
#[derive(Default)]
pub struct RasterCaptures {
    env: Cell<Option<Rc<RasterEnv>>>,
    buffers: Cell<Option<Box<ReplayBuffers>>>,
}

impl RasterCaptures {
    /// Forget the shared environment and the replay buffers.
    pub fn clear(&self) {
        self.env.take();
        self.buffers.take();
    }

    /// `env` without the reference this scratch keeps for later siblings, when no one else
    /// holds it: its replay then takes the captured stacks over instead of copying them.
    fn release(&self, env: Rc<RasterEnv>) -> Result<RasterEnv, Rc<RasterEnv>> {
        if Rc::strong_count(&env) == 2 {
            match self.env.take() {
                Some(kept) if Rc::ptr_eq(&kept, &env) => drop(kept),
                kept => self.env.set(kept),
            }
        }
        Rc::try_unwrap(env)
    }
}

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
            && b.ink.iter().zip(&window.next_frame.scene.ink).all(|(a, b)| a.same(b))
            && self.contexts == window.raster_frame.contexts
    }
}

/// Buffers of a finished replay of a shared environment, empty, with their capacity: the
/// next replay of a shared environment fills them instead of allocating.
#[derive(Default)]
struct ReplayBuffers {
    ids: SmallVec<[ElementId; 32]>,
    contexts: render_context::Snapshot,
    rendered: Vec<EntityId>,
    offsets: Vec<Point<Pixels>>,
    transforms: Vec<TransformationMatrix>,
    masks: Vec<ContentMask<Pixels>>,
    images: Vec<AnyImageCache>,
    ink: Vec<InkScope>,
    raster_contexts: render_context::Snapshot,
}

impl ReplayBuffers {
    /// The stacks of `env` copied into these buffers.
    fn fill(&mut self, env: &RasterEnv, own: &[ElementId]) -> PaintCtx {
        let b = &env.base;
        self.ids.extend(b.element_id_stack.iter().chain(own).cloned());
        b.render_contexts.clone_into(&mut self.contexts);
        self.rendered.extend_from_slice(&b.rendered_entity_stack);
        self.offsets.extend_from_slice(&b.element_offset_stack);
        self.transforms.extend_from_slice(&b.transformation_stack);
        self.masks.extend_from_slice(&b.content_mask_stack);
        self.images.extend(b.image_cache_stack.iter().cloned());
        self.ink.extend(b.ink.iter().cloned());
        env.contexts.clone_into(&mut self.raster_contexts);
        PaintCtx {
            identity: b.identity.clone(),
            layout_owner: b.layout_owner.clone(),
            element_id_stack: mem::take(&mut self.ids),
            text_style_stack: b.text_style_stack.clone(),
            render_contexts: mem::take(&mut self.contexts),
            rendered_entity_stack: mem::take(&mut self.rendered),
            element_offset_stack: mem::take(&mut self.offsets),
            exact_element_offset_depth: b.exact_element_offset_depth,
            transformation_stack: mem::take(&mut self.transforms),
            mask_scale: b.mask_scale,
            mask_map: b.mask_map,
            element_opacity: b.element_opacity,
            content_mask_stack: mem::take(&mut self.masks),
            image_cache_stack: mem::take(&mut self.images),
            ink: mem::take(&mut self.ink),
        }
    }

    /// Take the buffers back from a finished replay, empty, keeping their capacity.
    fn reclaim(&mut self, state: PaintState) {
        let PaintState { base, raster, .. } = state;
        self.ids = base.element_id_stack;
        self.contexts = base.render_contexts;
        self.rendered = base.rendered_entity_stack;
        self.offsets = base.element_offset_stack;
        self.transforms = base.transformation_stack;
        self.masks = base.content_mask_stack;
        self.images = base.image_cache_stack;
        self.ink = base.ink;
        self.raster_contexts = raster.contexts;
        self.ids.clear();
        self.contexts.clear();
        self.rendered.clear();
        self.offsets.clear();
        self.transforms.clear();
        self.masks.clear();
        self.images.clear();
        self.ink.clear();
        self.raster_contexts.clear();
    }
}

impl Window {
    /// Capture environment and raster placement without a scale override; siblings captured
    /// with the same `captures` share one environment.
    pub fn raster_paint_ctx(&self, captures: &RasterCaptures) -> RasterPaintCtx {
        let env = match captures.env.take() {
            Some(env) if env.matches(self) => env,
            _ => Rc::new(RasterEnv::capture(self, self.element_id_stack.len().saturating_sub(1))),
        };
        captures.env.set(Some(env.clone()));
        let prefix = env.base.element_id_stack.len();
        RasterPaintCtx {
            ids: self.element_id_stack[prefix..].iter().cloned().collect(),
            glyph_offset: self.raster_frame.glyph_offset,
            fill_transform: self.raster_frame.fill_transform,
            exact_bounds: self.raster_frame.exact_bounds,
            env,
        }
    }

    /// Replay captured raster placement in the original live scope. The last replay of an
    /// environment takes its stacks over (no copy, as a replay of a full snapshot does); a
    /// replay of a shared one copies them into buffers kept by `captures`.
    pub fn with_raster_paint_ctx<R>(
        &mut self,
        ctx: RasterPaintCtx,
        captures: &RasterCaptures,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintError> {
        let RasterPaintCtx { env, ids, glyph_offset, fill_transform, exact_bounds } = ctx;
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
            raster: RasterFrame { glyph_offset, fill_transform, exact_bounds, contexts },
            scale_factor: self.scale_factor,
            rem_size_overrides: self.rem_size_override_stack.clone(),
        };
        let result = self.replay_in(&mut state, f);
        if let Some(buffers) = buffers.as_mut() {
            buffers.reclaim(state);
        }
        if let Some(buffers) = buffers {
            captures.buffers.set(Some(buffers));
        }
        Ok(result)
    }
}