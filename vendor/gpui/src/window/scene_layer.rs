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
    pub(crate) source: Option<Rc<SceneLayer>>,
}

/// What `Window::record_scene_layer` made of the content.
pub enum RecordedLayer {
    /// The content's commands are in the layer and nowhere else yet: the
    /// caller paints the layer (`Window::paint_scene_layer`) where it wants it.
    Layer(LayerHandle),
    /// The content needs the frame scene (composition groups or hoisted
    /// backdrops): its commands were placed in the frame as if painted there,
    /// at other indices than recorded, so ranges recorded inside are void.
    Spliced,
    /// The paint environment allows no layer (a transform, opacity, mask,
    /// group, capture, open ink sinks or a layer being recorded): the content
    /// painted into the frame.
    Painted,
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
        {
            let previous = mem::replace(&mut self.layer_source, source);
            let (result, _) = self.with_restore(previous, |window, previous| {
                window.layer_source = previous.take();
            }, f);
            return (result, RecordedLayer::Painted);
        }
        let recording = Box::new(LayerRecording { bindings: Vec::new(), boxes: Vec::new(), source });
        // The frame's scene and no recording wait in `state` while `f`
        // paints into a new scene; restoring swaps them back, on unwind too.
        let mut state = (Scene::default(), Some(recording));
        mem::swap(&mut self.next_frame.scene, &mut state.0);
        mem::swap(&mut self.layer_recording, &mut state.1);
        let (result, (scene, recording)) = self.with_restore(state, |window, state| {
            mem::swap(&mut window.next_frame.scene, &mut state.0);
            mem::swap(&mut window.layer_recording, &mut state.1);
        }, f);
        let (bindings, boxes) = recording.map_or_else(Default::default, |r| (r.bindings, r.boxes));
        if !scene.groups.is_empty() || !scene.hoisted_backdrops.is_empty() {
            let scene = self.adopt_scene(scene);
            self.next_frame.scene.replay_record(&scene);
            return (result, RecordedLayer::Spliced);
        }
        (result, RecordedLayer::Layer(LayerHandle::new(SceneLayer::finish(scene, bindings, boxes))))
    }

    /// Draw `layer` here: one command in the frame, ordered like one
    /// primitive covering the layer's ink within the content mask.
    pub fn paint_scene_layer(&mut self, layer: &LayerHandle) {
        self.invalidator.debug_assert_paint();
        let mask = self.content_mask().scale(self.scale_factor());
        let bounds = layer.content().bounds().map_or_else(Bounds::default, |b| b.intersect(&mask.bounds));
        self.next_frame.scene.insert_layer(layer, bounds);
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
        recording.bindings.push(Binding {
            owner: self.paint_owner,
            value,
            quad: before as u32,
            op: (scene.paint_operations.len() - 1) as u32,
            opacity,
        });
    }

    /// Whether the rendered frame shows nothing new on screen: it repeats the
    /// frame before it except values written into layers outside the window
    /// (`Scene::damage`), and the platform shows that frame already. Then it
    /// needs neither raster nor present.
    pub(crate) fn present_unneeded(&self) -> bool {
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
