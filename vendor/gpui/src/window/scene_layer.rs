//! KaminIDE patch: recording retained scene layers (`scene::layer`) and
//! binding the values a host may write into them.
//!
//! A host paints its content into a layer once and hands the layer to each
//! later frame (`paint_scene_layer`); a frame that replays the host's paint
//! range copies the layer's one command, not its primitives. Ranges recorded
//! inside the layer (a host's inner records) index the layer's scene: a later
//! recording names that layer as its `source`, from which `reuse_paint` copies.

use super::*;
use crate::scene::{Binding, BoxRange, LayerHandle, LayerValue, SceneLayer};

/// A layer being recorded: the bindings its scopes painted and the layer
/// the previous recording of the same content made.
pub(crate) struct LayerRecording {
    pub(crate) bindings: Vec<Binding>,
    pub(crate) boxes: Vec<BoxRange>,
    /// Owners whose paint reached the layer outside their runs
    /// (`scene::layer::placed`): the layer holds none of their runs.
    pub(crate) unheld: Vec<u32>,
    pub(crate) source: Option<Rc<SceneLayer>>,
}

/// What `Window::record_scene_layer` made of the content.
pub enum RecordedLayer {
    /// The content's commands are in the layer and nowhere else yet: the
    /// caller paints the layer (`Window::paint_scene_layer`) where it wants it.
    /// Content that needs the frame scene (groups naming the frame's scroll
    /// nodes or 3D planes, hoisted backdrops) is a layer too, drawn by a copy
    /// (`SceneLayer::framed`): writes and box runs address it as any layer.
    Layer(LayerHandle),
    /// The paint environment allows no layer (a transform, opacity, mask,
    /// group, capture, open ink sinks or a layer being recorded): the content
    /// painted into the frame.
    Painted(NoLayer),
}

/// Why `record_scene_layer` made no layer, for DevTools: a stable reason and
/// the line that decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoLayer {
    /// `environment`, `ink`, `nested` or `disabled`.
    pub reason: &'static str,
    /// The line that decided.
    pub site: &'static core::panic::Location<'static>,
}

impl NoLayer {
    #[track_caller]
    fn at(reason: &'static str) -> Self {
        Self { reason, site: core::panic::Location::caller() }
    }
}

impl Window {
    /// Paint `f` into a new scene layer. `source` is the layer the previous
    /// recording of this content made: ranges recorded in it replay from it.
    pub fn record_scene_layer<R>(
        &mut self,
        source: Option<&LayerHandle>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, RecordedLayer) {
        self.invalidator.debug_assert_paint();
        let scene = &self.next_frame.scene;
        let source = source.map(LayerHandle::content);
        if !self.retained_root_environment()
            || !scene.ink.is_empty()
            || scene.ink_scope != 0
            || self.layer_recording.is_some()
            || self.scene_layers_disabled()
        {
            let why = match () {
                _ if !self.retained_root_environment() => NoLayer::at("environment"),
                _ if !scene.ink.is_empty() || scene.ink_scope != 0 => NoLayer::at("ink"),
                _ if self.layer_recording.is_some() => NoLayer::at("nested"),
                _ => NoLayer::at("disabled"),
            };
            let previous = mem::replace(&mut self.layer_source, source);
            let (result, _) = self.with_restore(previous, |window, previous| {
                window.layer_source = previous.take();
            }, f);
            return (result, RecordedLayer::Painted(why));
        }
        // The content painted again is about the size of its last recording:
        // its lists start at that size instead of growing by copies.
        let mut scene = Scene::default();
        let (mut bindings, mut boxes) = (Vec::new(), Vec::new());
        if let Some(last) = &source {
            // KaminIDE patch: what the last frame drew of this content, for
            // the comparison of a scroll step (`scene::damage::shift`).
            self.next_frame.scene.keep_layer_source(last);
            scene.reserve_like(&last.scene);
            let (bound, runs) = last.recorded_sizes();
            bindings.reserve_exact(bound);
            boxes.reserve_exact(runs);
        }
        crate::scene::no_run::start();
        let recording = Box::new(LayerRecording { bindings, boxes, unheld: Vec::new(), source });
        // The frame's scene and no recording wait in `state` while `f`
        // paints into a new scene; restoring swaps them back, on unwind too.
        let mut state = (scene, Some(recording));
        mem::swap(&mut self.next_frame.scene, &mut state.0);
        mem::swap(&mut self.layer_recording, &mut state.1);
        let (result, (scene, recording)) = self.with_restore(state, |window, state| {
            mem::swap(&mut window.next_frame.scene, &mut state.0);
            mem::swap(&mut window.layer_recording, &mut state.1);
        }, f);
        let (mut bindings, mut boxes, unheld) =
            recording.map_or_else(Default::default, |r| (r.bindings, r.boxes, r.unheld));
        crate::scene::release_unheld(&mut bindings, &mut boxes, unheld);
        // A layer keeps the groups and hoisted backdrops it made, even when
        // they name the frame's scroll nodes or 3D planes: a frame draws such
        // a layer by a copy (`grouped_layer`), and the layer stays the one
        // target of writes and box runs and the source of the ranges recorded
        // inside it. A layer records at the frame's root only, outside any
        // paint record's segment capture, so the copy binds no segments.
        debug_assert!(self.next_frame.scene.segment_capture.is_none(), "a layer inside a segment capture");
        (result, RecordedLayer::Layer(LayerHandle::new(SceneLayer::finish(scene, bindings, boxes))))
    }

    /// Tests paint without scene layers (`Window::set_scene_layers`).
    fn scene_layers_disabled(&self) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        return self.scene_layers_off;
        #[cfg(not(any(test, feature = "test-support")))]
        false
    }

    /// Draw `layer` here: one command in the frame, ordered like one
    /// primitive covering the layer's ink within the content mask. A layer
    /// that kept groups or backdrops is copied instead (`paint_grouped_layer`).
    pub fn paint_scene_layer(&mut self, layer: &LayerHandle) {
        self.invalidator.debug_assert_paint();
        let content = layer.content();
        if content.copied() {
            return self.paint_grouped_layer(layer, &content, true);
        }
        let mask = self.content_mask().scale(self.scale_factor());
        let bounds = layer.content().bounds().map_or_else(Bounds::default, |b| b.intersect(&mask.bounds));
        self.next_frame.scene.insert_layer(layer, bounds);
    }

    /// Draw again a layer that kept groups and was written since the frame
    /// that drew it (`SceneLayer::copied`): its last copy in the
    /// rendered frame is stale, a replay of it would show the old content.
    pub fn redraw_scene_layer(&mut self, layer: &LayerHandle) {
        self.invalidator.debug_assert_paint();
        let content = layer.content();
        if !content.copied() {
            return self.paint_scene_layer(layer);
        }
        self.paint_grouped_layer(layer, &content, false);
    }

    /// Paint `f` with `owner` (0: none) naming the values its painters bind.
    pub fn with_paint_owner<R>(&mut self, owner: u32, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.layer_recording.is_none() && self.paint_owner == 0 {
            return f(self);
        }
        let previous = mem::replace(&mut self.paint_owner, owner);
        self.with_restore(previous, |window, previous| window.paint_owner = *previous, f).0
    }

    /// Paint `f` binding the quads it paints to `value` of the paint owner.
    pub fn with_paint_value<R>(&mut self, value: LayerValue, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.layer_recording.is_none() || self.paint_owner == 0 {
            return f(self);
        }
        let previous = self.paint_value.replace(value);
        self.with_restore(previous, |window, previous| window.paint_value = *previous, f).0
    }

    /// Bind the quad `paint_quad` just inserted, if it inserted one (`before`:
    /// the quad count before) inside a binding scope of a layer recording.
    pub(crate) fn bind_painted_quad(&mut self, before: usize, opacity: f32) {
        let (Some(value), Some(recording)) = (self.paint_value, self.layer_recording.as_mut()) else {
            return;
        };
        let scene = &self.next_frame.scene;
        if self.paint_owner == 0 || scene.quads.len() != before + 1 {
            return;
        }
        // A group paints into a scene of its own (`with_isolated_scene`, the
        // groups of a paint plan), composited into the layer as one result:
        // its quads are not the layer's, and a binding to one would index
        // another scene's list.
        if scene.in_group && scene.book.record_runs.is_none() {
            crate::scene::layer_counters::GROUP_BINDS_SKIPPED
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        let binding = Binding {
            owner: self.paint_owner,
            value,
            quad: before as u32,
            op: (scene.paint_operations.len() - 1) as u32,
            opacity,
        };
        // A paint record keeps the binding for its placements (`scene::layer::placed`).
        match self.next_frame.scene.book.record_runs.as_mut() {
            Some(runs) => runs.bindings.push(binding),
            None => recording.bindings.push(binding),
        }
    }

    /// Whether the rendered frame shows nothing new on screen: it repeats the
    /// frame before it except values written into layers outside the window
    /// (`Scene::damage`), and the platform shows that frame already. Then it
    /// needs neither raster nor present.
    pub(crate) fn present_unneeded(&self) -> bool {
        if self.frame_readback && !self.unpresented_readback {
            return false;
        }
        let Some(damage) = self.rendered_frame.scene.damage() else {
            return false;
        };
        let screen = Bounds { origin: Point::default(), size: self.viewport_size }
            .scale(self.scale_factor());
        let unseen = damage.is_none_or(|d| d.intersect(&screen).is_empty());
        unseen && self.frame_presented && self.platform_window.last_frame_presented()
    }

    /// Present the next drawn frame whatever it changed on screen: a reader of
    /// the presented pixels (a capture, a readback) waits for it.
    pub fn require_presentation(&mut self) {
        self.presentation_required = true;
    }

    /// Present the frame just drawn, unless it changed nothing on screen and
    /// the platform did not ask for a presentation (`present_unneeded`).
    pub(crate) fn present_drawn(&mut self, required: bool) {
        let required = required || std::mem::take(&mut self.presentation_required);
        if !required && self.present_unneeded() {
            crate::scene::layer_counters::PRESENTS_SKIPPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(mut at) = crate::scene::layer_counters::LAST_SKIP.lock() {
                *at = Some(std::time::Instant::now());
            }
            self.needs_present.set(false);
            return;
        }
        self.present();
    }

    /// The scene `reuse_paint` copies paint ranges from: the source layer of
    /// the content being painted, else the rendered frame's.
    pub(crate) fn replay_source(&self) -> Option<Rc<SceneLayer>> {
        match &self.layer_recording {
            Some(recording) => recording.source.clone(),
            None => self.layer_source.clone(),
        }
    }
}
