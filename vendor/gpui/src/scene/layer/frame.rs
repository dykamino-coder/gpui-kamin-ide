//! KaminIDE patch: layers inside a frame scene (`scene::layer`).
//!
//! The frame takes one command per layer (`PaintOperation::Layer`) that names
//! the host's layer weakly, and one slot that holds the content it draws
//! strongly from its draw to its present (`Scene::release_layers`). A frame
//! that replays the command reads the host's current content.

use super::{LayerCell, LayerHandle, SceneLayer, layer_counters};
use crate::scene::{BOTTOM_LAYER, Bounds, DrawOrder, ORDER_STEP, PaintOperation, ScaledPixels, Scene};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering::Relaxed;

/// A layer in a frame scene: its order there, the host's layer and, from the
/// frame's draw to its present, the content the frame draws.
pub(crate) struct LayerSlot {
    pub(crate) order: DrawOrder,
    pub(crate) cell: Weak<LayerCell>,
    pub(crate) held: Option<Rc<SceneLayer>>,
}

/// The command of a layer in a frame scene: the host's layer, its bounds in
/// the frame (clipped by the content mask) and its order there.
#[derive(Clone)]
pub(crate) struct LayerOp {
    pub(crate) cell: Weak<LayerCell>,
    pub(crate) bounds: Bounds<ScaledPixels>,
    pub(crate) order: DrawOrder,
}

impl Scene {
    /// Add the current content of `layer` to this scene, ordered like one
    /// primitive of `bounds` (already clipped by the content mask).
    pub(crate) fn insert_layer(&mut self, layer: &LayerHandle, bounds: Bounds<ScaledPixels>) {
        if bounds.is_empty() {
            return;
        }
        crate::window::ink_add(&self.ink, self.ink_scope, bounds);
        crate::window::scoped_ink(self.ink_scope, |sink| sink.add_in(self.ink_scope, bounds));
        let order = match self.layer_stack.last().copied() {
            Some(BOTTOM_LAYER) => {
                self.bottom_seq = (self.bottom_seq + 1).min(ORDER_STEP - 1);
                ORDER_STEP + self.bottom_seq
            }
            Some(order) => order,
            None => self.tree_mut().insert(bounds) * ORDER_STEP,
        };
        // A recorded layer is new paint: its damage is all of it.
        layer.0.damage.take();
        self.fresh += 1;
        let cell = Rc::downgrade(&layer.0);
        self.paint_operations.push(PaintOperation::Layer(LayerOp { cell: cell.clone(), bounds, order }));
        self.layers.push(LayerSlot { order, cell, held: Some(layer.content()) });
    }

    /// Add the slot of a replayed layer command (its order already set);
    /// `false` when the host dropped the layer, which then draws nothing.
    pub(super) fn push_layer_slot(&mut self, op: &LayerOp) -> bool {
        let Some(cell) = op.cell.upgrade() else {
            return false;
        };
        let held = cell.content.borrow().clone();
        if let Some(rect) = cell.damage.take() {
            self.layer_damage = Some(self.layer_damage.map_or(rect, |d| d.union(&rect)));
        }
        self.layers.push(LayerSlot { order: op.order, cell: op.cell.clone(), held: Some(held) });
        true
    }

    /// The content of layer `index` (`PrimitiveBatch::Layer`), while the scene
    /// holds it: its scene, id and version.
    pub fn layer(&self, index: usize) -> Option<&SceneLayer> {
        self.layers.get(index)?.held.as_deref()
    }

    /// Number of layers this scene draws.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// The scene was presented: let the content of its layers go, so a write
    /// before the next draw does not copy it.
    pub(crate) fn release_layers(&mut self) {
        for slot in &mut self.layers {
            slot.held = None;
        }
    }

    /// Hold the layers' current content again to present this scene once more.
    pub(crate) fn hold_layers(&mut self) {
        for slot in &mut self.layers {
            if slot.held.is_none() {
                slot.held = slot.cell.upgrade().map(|cell| cell.content.borrow().clone());
            }
        }
    }

    /// Primitives of every kind, the layers' included.
    pub fn primitive_count(&self) -> usize {
        self.shadows.len()
            + self.quads.len()
            + self.paths.len()
            + self.underlines.len()
            + self.monochrome_sprites.len()
            + self.subpixel_sprites.len()
            + self.polychrome_sprites.len()
            + self.surfaces.len()
            + self.layers.iter().filter_map(|s| s.held.as_ref()).map(|l| l.scene.primitive_count()).sum::<usize>()
    }

    /// `finish`, returning where each quad moved (`moved[old] == new`).
    pub(crate) fn finish_tracking_quads(&mut self) -> Vec<u32> {
        let mut by_order: Vec<u32> = (0..self.quads.len() as u32).collect();
        // Stable, as `finish`'s sort: equal orders keep paint order.
        by_order.sort_by_key(|&at| self.quads[at as usize].order);
        let quads = by_order.iter().map(|&at| self.quads[at as usize]).collect();
        self.quads = quads;
        let mut moved = vec![0; by_order.len()];
        for (new, &old) in by_order.iter().enumerate() {
            moved[old as usize] = new as u32;
        }
        self.finish();
        moved
    }

    /// A copy of a finished layer scene (a write's copy on write).
    pub(super) fn copy_finished(&self) -> Scene {
        Scene {
            paint_operations: self.paint_operations.clone(),
            primitive_bounds: self.primitive_bounds.clone(),
            shadows: self.shadows.clone(),
            quads: self.quads.clone(),
            paths: self.paths.clone(),
            underlines: self.underlines.clone(),
            monochrome_sprites: self.monochrome_sprites.clone(),
            subpixel_sprites: self.subpixel_sprites.clone(),
            polychrome_sprites: self.polychrome_sprites.clone(),
            surfaces: self.surfaces.clone(),
            ..Scene::default()
        }
    }

    /// Order the layers for batching (`finish`).
    pub(crate) fn finish_layers(&mut self) {
        let layers = &self.layers;
        self.layer_order.clear();
        self.layer_order.extend(0..layers.len() as u32);
        self.layer_order.sort_by_key(|&at| layers[at as usize].order);
    }

    /// Count `n` primitives copied from an earlier scene.
    pub(crate) fn count_copied(n: usize) {
        layer_counters::PRIMITIVES_COPIED.fetch_add(n as u64, Relaxed);
    }
}
