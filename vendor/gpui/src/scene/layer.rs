//! KaminIDE patch: a retained scene layer.
//!
//! A host paints content once into a layer (`Window::record_scene_layer`),
//! keeps the layer across frames and hands it to every frame by `Rc`
//! (`Window::paint_scene_layer`): the frame scene takes one command for it
//! and neither copies nor sorts its primitives, which were sorted once, when
//! the layer was recorded. A renderer draws a layer from buffers it derives
//! from the layer and keeps per layer version: a cache, not a second list.
//!
//! Between frames the host writes values into the layer through bindings
//! (`LayerHandle::set_background`): a binding scope (`Window::with_paint_owner`
//! and the value scope of the painter) names the primitives it painted, so a
//! write finds them by bisection, raises the version and returns the damaged
//! rectangle. A frame holds the content it draws strongly only from its draw
//! to its present, and the host's handle weakly after: a write copies the
//! content only while a frame or snapshot still holds it, and a frame that
//! replays the layer's command later reads the host's current content.

mod frame;

use super::{Background, Bounds, Hsla, PaintOperation, Primitive, ScaledPixels, Scene};
pub(crate) use frame::{LayerOp, LayerSlot};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// A value a host may write into a layer without painting again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LayerValue {
    /// The fill of a box's background quad (`Style::paint_box`).
    Background = 1,
}

/// A quad a binding scope painted: the scope's owner and value, the quad and
/// its command, and the opacity its colors were painted under.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Binding {
    pub(crate) owner: u32,
    pub(crate) value: LayerValue,
    pub(crate) quad: u32,
    pub(crate) op: u32,
    pub(crate) opacity: f32,
}

/// A finished scene with its version and bindings.
pub struct SceneLayer {
    pub(crate) scene: Scene,
    id: u64,
    version: u64,
    /// Union of the primitives' bounds within their content masks.
    bounds: Option<Bounds<ScaledPixels>>,
    /// By owner, then value, then quad: a write finds its range by bisection.
    bindings: Vec<Binding>,
}

/// Process-wide counters of layer work (`frame_perf`-style, read by benches).
pub mod layer_counters {
    use std::sync::atomic::AtomicU64;
    /// Layers recorded.
    pub static RECORDS: AtomicU64 = AtomicU64::new(0);
    /// Copies a write made because a frame or snapshot still held the content.
    pub static COW_CLONES: AtomicU64 = AtomicU64::new(0);
    /// Primitive values written into layers.
    pub static VALUES_WRITTEN: AtomicU64 = AtomicU64::new(0);
    /// Primitives frames copied from earlier scenes (`Scene::replay` and
    /// the record replays): what a layer saves.
    pub static PRIMITIVES_COPIED: AtomicU64 = AtomicU64::new(0);
}

/// The host's layer: its current content, which a write replaces.
#[derive(Clone)]
pub struct LayerHandle(pub(crate) Rc<LayerCell>);

pub(crate) struct LayerCell(pub(crate) RefCell<Rc<SceneLayer>>);

impl Clone for SceneLayer {
    /// A write's copy: only while a frame or snapshot holds the content.
    fn clone(&self) -> Self {
        layer_counters::COW_CLONES.fetch_add(1, Relaxed);
        Self {
            scene: self.scene.copy_finished(),
            id: self.id,
            version: self.version,
            bounds: self.bounds,
            bindings: self.bindings.clone(),
        }
    }
}

impl SceneLayer {
    /// Finish `scene`, painted with `bindings` (quad indices in paint order),
    /// into a layer: the scene is sorted once and the bindings follow their quads.
    pub(crate) fn finish(mut scene: Scene, mut bindings: Vec<Binding>) -> Self {
        let moved = scene.finish_tracking_quads();
        for binding in &mut bindings {
            binding.quad = moved[binding.quad as usize];
        }
        bindings.sort_by_key(|b| (b.owner, b.value, b.quad));
        let bounds = scene
            .paint_operations
            .iter()
            .filter_map(|operation| match operation {
                PaintOperation::Primitive(p) => {
                    Some(super::placed_bounds(p).intersect(&p.content_mask().bounds))
                }
                _ => None,
            })
            .filter(|b| !b.is_empty())
            .reduce(|a, b| a.union(&b));
        static NEXT: AtomicU64 = AtomicU64::new(1);
        layer_counters::RECORDS.fetch_add(1, Relaxed);
        Self { scene, id: NEXT.fetch_add(1, Relaxed), version: 0, bounds, bindings }
    }

    /// Union of the primitives' bounds within their content masks; `None`
    /// when the layer draws nothing.
    pub fn bounds(&self) -> Option<Bounds<ScaledPixels>> {
        self.bounds
    }

    /// Identity of the recording; a copy on write keeps it.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Raised by every write.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The layer's finished scene.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    fn bound(&self, owner: u32, value: LayerValue) -> std::ops::Range<usize> {
        let start = self.bindings.partition_point(|b| (b.owner, b.value) < (owner, value));
        let end = self.bindings.partition_point(|b| (b.owner, b.value) <= (owner, value));
        start..end
    }
}

impl LayerHandle {
    pub(crate) fn new(layer: SceneLayer) -> Self {
        Self(Rc::new(LayerCell(RefCell::new(Rc::new(layer)))))
    }

    /// The current content.
    pub fn content(&self) -> Rc<SceneLayer> {
        self.0.0.borrow().clone()
    }

    /// Whether two handles name the same layer.
    pub fn same_layer(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Write the background of `owner`: every quad its background scope
    /// painted takes `to`, when every one of them was painted in `from` (under
    /// the opacity it was painted with). Returns the damaged rectangle in
    /// device pixels, or `None` with nothing written when the owner has no
    /// such quad or one painted something else, or when `to` is transparent
    /// (the painter paints no quad then): the host paints again.
    pub fn set_background(&self, owner: u32, from: Hsla, to: Hsla) -> Option<Bounds<ScaledPixels>> {
        if to.a == 0. {
            return None;
        }
        let mut current = self.0.0.borrow_mut();
        let range = current.bound(owner, LayerValue::Background);
        let painted = |b: &Binding| Background::from(from).opacity(b.opacity);
        let all = !range.is_empty()
            && current.bindings[range.clone()].iter().all(|b| {
                current.scene.quads.get(b.quad as usize).is_some_and(|q| q.background == painted(b))
            });
        if !all {
            return None;
        }
        let layer = Rc::make_mut(&mut current);
        let mut damage: Option<Bounds<ScaledPixels>> = None;
        let mut edge = to;
        edge.a = 0.;
        for at in range.clone() {
            let b = layer.bindings[at];
            let fill = Background::from(to).opacity(b.opacity);
            let border = edge.opacity(b.opacity);
            if let Some(quad) = layer.scene.quads.get_mut(b.quad as usize) {
                quad.background = fill;
                quad.border_color = border;
                let rect = quad.bounds.intersect(&quad.content_mask.bounds);
                damage = Some(damage.map_or(rect, |d| d.union(&rect)));
            }
            if let Some(PaintOperation::Primitive(Primitive::Quad(quad))) =
                layer.scene.paint_operations.get_mut(b.op as usize)
            {
                quad.background = fill;
                quad.border_color = border;
            }
        }
        layer.version += 1;
        layer_counters::VALUES_WRITTEN.fetch_add(range.len() as u64, Relaxed);
        damage
    }
}

#[cfg(test)]
mod tests;
