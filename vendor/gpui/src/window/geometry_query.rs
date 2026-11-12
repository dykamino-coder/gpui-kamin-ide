//! Изолированный синхронный проход раскладки без публикации кадра окна.

use super::*;

impl Window {
    /// Inspect current element states without taking or mutating them. A state
    /// already rebuilt in the next frame takes precedence over its old value.
    pub fn read_element_states<S: 'static>(
        &self,
        mut read: impl FnMut(&GlobalElementId, &S),
    ) {
        let ty = TypeId::of::<S>();
        for (key, state) in &self.rendered_frame.element_states {
            if key.1 == ty && !self.next_frame.element_states.contains_key(key) {
                if let Some(Some(state)) = state.inner.downcast_ref::<Option<S>>() {
                    read(&key.0, state);
                }
            }
        }
        for (key, state) in &self.next_frame.element_states {
            if key.1 == ty {
                if let Some(Some(state)) = state.inner.downcast_ref::<Option<S>>() {
                    read(&key.0, state);
                }
            }
        }
    }

    /// Run an explicit layout query in a temporary arena and layout tree.
    /// Hitboxes, deferred draws and element states belong to the query only.
    /// The callback may request layout and prepaint, but must not paint.
    pub fn with_layout_query<R>(
        &mut self,
        cx: &mut App,
        query: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> R {
        let state = Box::new(self.snapshot_paint_state());
        let frame = || Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone()));
        let next = mem::replace(&mut self.next_frame, frame());
        let rendered = mem::replace(&mut self.rendered_frame, frame());
        let phase = self.invalidator.inner.borrow().draw_phase;
        let hit = self.hit_scope.take();
        let inputs = mem::take(&mut self.layout_inputs);
        let retry = self.layout_retry;
        let callbacks = mem::take(&mut self.next_frame_callbacks);
        let autoscroll = self.requested_autoscroll.take();
        let _contexts = self.render_contexts.enter_snapshot(Default::default());
        self.invalidator.set_phase(DrawPhase::Prepaint);
        self.element_id_stack.clear();
        self.text_style_stack = Default::default();
        self.rendered_entity_stack.clear();
        self.element_offset_stack.clear();
        self.transformation_stack.clear();
        self.content_mask_stack.clear();
        self.image_cache_stack.clear();
        self.rem_size_override_stack.clear();
        self.raster_frame = Default::default();
        self.mask_scale = None;
        self.mask_map = None;
        self.element_opacity = 1.;
        self.exact_element_offset_depth = 0;
        let result = self.with_restore(
            (next, rendered, state, phase, hit, inputs, retry, callbacks, autoscroll),
            |window, saved| {
                mem::swap(&mut window.next_frame, &mut saved.0);
                mem::swap(&mut window.rendered_frame, &mut saved.1);
                saved.2.restore(window);
                window.invalidator.set_phase(saved.3);
                window.hit_scope = saved.4.take();
                mem::swap(&mut window.layout_inputs, &mut saved.5);
                window.layout_retry = saved.6;
                mem::swap(&mut window.next_frame_callbacks, &mut saved.7);
                window.requested_autoscroll = saved.8.take();
            },
            |window| {
                with_temporary_element_arena(|| {
                    window.with_nested_layout(|window| {
                        // Keep the nested pass's panic cleanup away from the
                        // enclosing text frame. Restore all query state first.
                        window.with_paint_pass(|window| {
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                window.with_layout_projections(|window| query(window, cx))
                            }))
                        })
                    })
                })
            },
        ).0;
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Finish positioned descendants queued by a synchronous layout query.
    pub fn finish_layout_query(&mut self, cx: &mut App) {
        self.prepaint_deferred_draws(cx);
    }

    /// Resolved border and padding edges in CSS pixels, before pixel snapping.
    pub fn layout_box_edges(&self, id: LayoutId) -> (Edges<Pixels>, Edges<Pixels>) {
        self.layout_engine.as_ref().expect("layout engine during query")
            .layout_box_edges(id, self.scale_factor())
    }
}
