//! KaminIDE patch: the own paint of a box in a scene layer, and its later
//! recording into the box's run (`scene::layer::boxes`).
//!
//! A host repaints a box whose painted values changed and whose layout did
//! not by painting only the box's own commands again, in the paint state the
//! first recording saved; the layer replaces the box's
//! run with them (`SceneLayer::replace_box`). Neither the box's descendants
//! nor its neighbours paint again, and the frame that shows the layer copies
//! none of its primitives.

use super::scene_layer::LayerRecording;
use super::*;
use crate::scene::{BoxEnv, LayerHandle, LayerPainter, Primitive, RunPainter};
use crate::{PaintTraversal, Style};

impl Window {
    /// Record the painter that produced the run alongside its geometry and commands.
    pub(crate) fn with_run<R>(
        &mut self,
        owner: u32,
        bounds: Bounds<Pixels>,
        painter: RunPainter,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if owner == 0 || self.layer_recording.is_none() {
            return self.with_paint_owner(owner, f);
        }
        let env = BoxEnv {
            bounds,
            mask: self.content_mask(),
            transformation: self.current_transformation(),
            opacity: self.element_opacity(),
            rem_size: self.rem_size(),
            fill: None,
            layer: None,
            painter,
        };
        let area = self.box_area(&env);
        if !self.next_frame.scene.open_box(owner, area, env) {
            // A partial set of runs cannot represent all paint for this owner.
            self.unhold_owner(owner);
            return self.with_paint_owner(owner, f);
        }
        let result = self.with_paint_owner(owner, f);
        let range = self.next_frame.scene.close_box();
        // A paint record keeps the run for its placements (`scene::layer::placed`).
        if let (Some(range), Some(runs)) = (&range, self.next_frame.scene.book.record_runs.as_mut()) {
            runs.boxes.push(range.clone());
        } else if let (Some(range), Some(recording)) = (range, self.layer_recording.as_mut()) {
            recording.boxes.push(range);
        }
        result
    }

    /// Where the host places the fill of `style` in `bounds`; a box recorded
    /// again takes the bounds its first recording placed, the host's raster
    /// frame of that paint being gone.
    /// A first recording places every fill on its own bounds: a box painted
    /// per fragment (`BoxPainter`) has one fill per fragment.
    pub(crate) fn box_fill_bounds(&self, style: &Style, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        if let Some(fill) = self.box_fill {
            return fill;
        }
        self.render_context::<PaintTraversal>()
            .map_or(bounds, |host| (host.fill_bounds)(style, bounds, self))
    }

    /// Keep the fill bounds of the box whose own paint is open, painted or
    /// not. A second fill (the box painted per fragment) is no one fill a
    /// later recording could restore: the run is then not replaceable.
    pub(crate) fn note_box_fill(&mut self, style: &Style, bounds: Bounds<Pixels>) {
        if self.box_fill.is_none() && self.next_frame.scene.box_open() {
            let fill = self.box_fill_bounds(style, bounds);
            self.next_frame.scene.note_open_box_fill(fill);
        }
    }

    /// The border box of `env` placed on the screen, in device pixels.
    fn box_area(&self, env: &BoxEnv) -> Bounds<ScaledPixels> {
        let scale = self.scale_factor();
        let quad = Quad {
            transformation: env.transformation,
            bounds: env.bounds.scale(scale),
            content_mask: env.mask.scale(scale),
            ..Default::default()
        };
        crate::scene::ink_of(&Primitive::Quad(quad))
    }

    /// Paint the own commands of box `owner` in `layer` again with `paint`
    /// (given the border box), in the paint state of its recording, and
    /// replace its runs with them, ordered as a paint from scratch orders them. `None`, with the layer as it
    /// was for the box's runs not yet replaced, when the box has no
    /// replaceable run or the new commands would need another order; then
    /// the host paints the layer again. `Some(moved)`: whether paint
    /// commands after a run moved (ranges recorded in the layer are void).
    pub fn rerecord_layer_box(
        &mut self,
        layer: &LayerHandle,
        owner: u32,
        cx: &mut App,
        mut paint: impl FnMut(&mut Self, &mut App, Bounds<Pixels>),
    ) -> Option<bool> {
        self.rerecord_runs(layer, owner, cx, LayerPainter::Box, |w, cx, env| paint(w, cx, env.bounds))
    }

    /// Record the runs of box `owner` again, each with `paint` in the paint
    /// state of its recording: the box's own painter's runs (`band` false,
    /// outside stacking layers) or the pieces of its band (`band`, in the
    /// stacking layer of their line).
    pub(crate) fn rerecord_runs(
        &mut self,
        layer: &LayerHandle,
        owner: u32,
        cx: &mut App,
        painter: LayerPainter,
        mut paint: impl FnMut(&mut Self, &mut App, &BoxEnv),
    ) -> Option<bool> {
        if self.layer_recording.is_some() || !layer.accepts(owner, painter) {
            return None;
        }
        let runs: Vec<usize> = {
            let content = layer.0.content.borrow();
            content.runs_of(owner).collect()
        };
        let mut moved = false;
        for at in runs {
            let (area, env, placed) = {
                let content = layer.0.content.borrow();
                let b = &content.boxes[at];
                (b.area, b.env, b.placed)
            };
            // A run in a stacking layer keeps the layer's order
            // (`SceneLayer::plan_layer_run`): only band pieces are there.
            let (scene, bindings) = self.paint_box_run(owner, area, env, cx, &mut paint);
            // A run placed from a paint record: the new commands are placed the same way.
            let (scene, bindings) = match placed {
                Some(placed) => crate::scene::place_run(scene, bindings, placed)?,
                None => (scene, bindings),
            };
            let mut current = layer.0.content.borrow_mut();
            let plan = current.check_box(at, &scene)?;
            let content = Rc::make_mut(&mut *current);
            let (damage, shifted) = content.replace_box(at, scene, bindings, plan);
            moved |= shifted;
            if let Some(rect) = damage {
                let old = layer.0.damage.get();
                layer
                    .0
                    .damage
                    .set(Some(old.map_or(rect, |d| d.union(&rect))));
            }
            crate::scene::layer_counters::BOXES_RERECORDED
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Some(moved)
    }

    /// Paint one run of box `owner` into a scratch scene, in `env`.
    fn paint_box_run(
        &mut self,
        owner: u32,
        area: Bounds<ScaledPixels>,
        env: BoxEnv,
        cx: &mut App,
        paint: &mut impl FnMut(&mut Self, &mut App, &BoxEnv),
    ) -> (Scene, Vec<crate::scene::Binding>) {
        let recording = Box::new(LayerRecording {
            bindings: Vec::new(),
            boxes: Vec::new(),
            unheld: Vec::new(),
            source: None,
        });
        let mut state = (Scene::default(), Some(recording));
        mem::swap(&mut self.next_frame.scene, &mut state.0);
        mem::swap(&mut self.layer_recording, &mut state.1);
        let phase = self.invalidator.phase();
        self.invalidator.set_phase(DrawPhase::Paint);
        self.content_mask_stack.push(env.mask);
        let opacity = mem::replace(&mut self.element_opacity, env.opacity);
        self.box_fill = env.fill;
        self.next_frame.scene.open_box(owner, area, env);
        self.with_rem_size(Some(env.rem_size), |window| {
            window.with_paint_owner(owner, |window| paint(window, cx, &env))
        });
        self.next_frame.scene.close_box();
        self.box_fill = None;
        self.element_opacity = opacity;
        self.content_mask_stack.pop();
        self.invalidator.set_phase(phase);
        mem::swap(&mut self.next_frame.scene, &mut state.0);
        mem::swap(&mut self.layer_recording, &mut state.1);
        let bindings = state.1.map_or_else(Vec::new, |r| r.bindings);
        (state.0, bindings)
    }
}
