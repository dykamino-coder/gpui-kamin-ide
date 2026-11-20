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
        self.book.fresh += 1;
        let cell = Rc::downgrade(&layer.0);
        self.paint_operations.push(PaintOperation::Layer(LayerOp { cell: cell.clone(), bounds, order }));
        self.book.layers.push(LayerSlot { order, cell, held: Some(layer.content()) });
    }

    /// Add the slot of a replayed layer command (its order already set);
    /// `false` when the host dropped the layer, which then draws nothing.
    pub(crate) fn push_layer_slot(&mut self, op: &LayerOp) -> bool {
        let Some(cell) = op.cell.upgrade() else {
            return false;
        };
        let held = cell.content.borrow().clone();
        if let Some(rect) = cell.damage.take() {
            self.book.layer_damage = Some(self.book.layer_damage.map_or(rect, |d| d.union(&rect)));
        }
        self.book.layers.push(LayerSlot { order: op.order, cell: op.cell.clone(), held: Some(held) });
        true
    }

    /// The content of layer `index` (`PrimitiveBatch::Layer`), while the scene
    /// holds it: its scene, id and version.
    pub fn layer(&self, index: usize) -> Option<&SceneLayer> {
        self.book.layers.get(index)?.held.as_deref()
    }

    /// The content of every layer this scene draws: what it holds, or after
    /// its present the host's current content (tests and snapshots read it).
    pub fn layer_contents(&self) -> Vec<Rc<SceneLayer>> {
        let current = |slot: &LayerSlot| slot.cell.upgrade().map(|cell| cell.content.borrow().clone());
        self.book.layers.iter().filter_map(|slot| slot.held.clone().or_else(|| current(slot))).collect()
    }

    /// Number of layers this scene draws.
    pub fn layer_count(&self) -> usize {
        self.book.layers.len()
    }

    /// The scene was presented: let the content of its layers go, so a write
    /// before the next draw does not copy it.
    pub(crate) fn release_layers(&mut self) {
        for slot in &mut self.book.layers {
            slot.held = None;
        }
    }

    /// Hold the layers' current content again to present this scene once more.
    pub(crate) fn hold_layers(&mut self) {
        for slot in &mut self.book.layers {
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
            + self.book.layers.iter().filter_map(|s| s.held.as_ref()).map(|l| l.scene.primitive_count()).sum::<usize>()
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
            book: Box::new(super::super::SceneBook { images: self.book.images.clone(), ..Default::default() }),
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
            // The copy keeps the scroll marks and the space ranges recorded in
            // the layer count in (`scroll_marks::SceneSpace`).
            scroll: super::super::scroll_marks::ScrollBook {
                marks: self.scroll.marks.clone(),
                space: self.scroll.space,
                ..Default::default()
            },
            ..Scene::default()
        }
    }

    /// Order the layers for batching (`finish`).
    pub(crate) fn finish_layers(&mut self) {
        let layers = &self.book.layers;
        self.book.layer_order.clear();
        self.book.layer_order.extend(0..layers.len() as u32);
        self.book.layer_order.sort_by_key(|&at| layers[at as usize].order);
    }

    /// What this finished frame changed against `previous`, the frame it
    /// replays (`damage`). Platform-neutral: a renderer or a CPU backend may
    /// raster only the damage, and a frame whose damage is off screen needs
    /// no raster at all.
    pub(crate) fn note_damage(&mut self, previous: &Scene) {
        let same = self.book.fresh == 0
            && self.groups.is_empty()
            && previous.groups.is_empty()
            && self.hoisted_backdrops.is_empty()
            && self.paint_operations.len() == previous.paint_operations.len()
            && self.book.layers.len() == previous.book.layers.len()
            && self.book.layers.iter().zip(&previous.book.layers).all(|(a, b)| {
                a.order == b.order && Weak::ptr_eq(&a.cell, &b.cell)
            });
        self.book.damage = same.then_some(self.book.layer_damage);
    }

    /// The device-pixel rectangle this frame changed against the frame
    /// before it: `Some(None)` — nothing, `Some(Some(r))` — only `r` (values
    /// written into its layers), `None` — unknown (new paint).
    pub fn damage(&self) -> Option<Option<Bounds<ScaledPixels>>> {
        self.book.damage
    }

    /// Count `n` primitives copied from an earlier scene.
    pub(crate) fn count_copied(n: usize) {
        layer_counters::PRIMITIVES_COPIED.fetch_add(n as u64, Relaxed);
    }
}
