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
//! (`LayerHandle::set_backgrounds`, `write`): a binding scope
//! (`Window::with_paint_owner` and the value scope of the painter) names the
//! primitives it painted, so a write finds them by bisection, raises the
//! version and returns the damaged rectangle. A box whose own paint changes
//! otherwise records its run of commands again under its key (`boxes`,
//! `splice`). A frame holds the content it draws strongly only from its draw
//! to its present, and the host's handle weakly after: a write copies the
//! content only while a frame or snapshot still holds it, and a frame that
//! replays the layer's command later reads the host's current content.

mod boxes;
mod frame;
mod finish;
mod order;
mod placed;
mod reorder;
mod splice;
mod write;

use super::{Bounds, ScaledPixels, Scene};
pub(crate) use boxes::{BoxEnv, BoxRange, OpenBox, ink_of};
pub(crate) use frame::{LayerOp, LayerSlot};
pub(crate) use placed::{Placed, PlacedMap, RecordRuns, place_run, release_unheld};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::Ordering::Relaxed;

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
    /// The runs of the boxes' own commands, in paint order.
    pub(crate) boxes: Vec<BoxRange>,
    /// Owners with no commands in this recording, after checking every copy.
    omitted: Vec<u32>,
    /// `(owner, index into boxes)`, sorted: a box's runs by bisection
    /// (`runs_of`), not a scan of every run of the layer. Built lazily.
    by_owner: std::cell::OnceCell<Vec<(u32, u32)>>,
}

/// Union of the placed bounds within their content masks of `operations`'
/// primitives, in command order: what `Scene::primitive_ink` keeps.
#[cfg(debug_assertions)]
fn ink_of_commands(operations: &[super::PaintOperation]) -> Option<Bounds<ScaledPixels>> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            super::PaintOperation::Primitive(p) => {
                Some(super::placed_bounds(p).intersect(&p.content_mask().bounds))
            }
            _ => None,
        })
        .filter(|b| !b.is_empty())
        .reduce(|a, b| a.union(&b))
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
    /// Runs of boxes' own commands recorded again (`Window::rerecord_layer_box`).
    pub static BOXES_RERECORDED: AtomicU64 = AtomicU64::new(0);
    /// Runs recorded again but refused because the layer cannot be ordered
    /// as a paint from scratch in place (3D commands, a retained layer inside
    /// it to move), so the host paints the layer again (`SceneLayer::plan_box`).
    pub static RUNS_REORDERED: AtomicU64 = AtomicU64::new(0);
    /// Runs recorded again that moved later commands to the buckets a paint
    /// from scratch gives them (`SceneLayer::move_later`).
    pub static RUNS_MOVED_LATER: AtomicU64 = AtomicU64::new(0);
    /// Bindings not made because the quad was painted into a group's scene.
    pub static GROUP_BINDS_SKIPPED: AtomicU64 = AtomicU64::new(0);
    /// Box runs of a paint record that could not be placed by one shift and
    /// clip (`scene::layer::placed`): their boxes paint the layer anew.
    pub static RUNS_UNPLACED: AtomicU64 = AtomicU64::new(0);
    /// Layers whose bindings or runs named another scene (`SceneLayer::finish`):
    /// they take no writes. Zero on every page, or a recording is wrong.
    pub static RECORDS_REFUSED: AtomicU64 = AtomicU64::new(0);
    /// Primitives frames copied from earlier scenes (`Scene::replay` and
    /// the record replays): what a layer saves.
    pub static PRIMITIVES_COPIED: AtomicU64 = AtomicU64::new(0);
    /// Frames neither rastered nor presented: nothing changed on screen.
    pub static PRESENTS_SKIPPED: AtomicU64 = AtomicU64::new(0);
    /// When the last frame skipped its presentation (`PRESENTS_SKIPPED`):
    /// the end of that frame for a bench.
    pub static LAST_SKIP: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
}

/// The host's layer: its current content, which a write replaces.
#[derive(Clone)]
pub struct LayerHandle(pub(crate) Rc<LayerCell>);

pub(crate) struct LayerCell {
    pub(crate) content: RefCell<Rc<SceneLayer>>,
    /// Union of the rectangles written since a frame last took the layer
    /// (`Scene::push_layer_slot`): that frame's damage from this layer.
    pub(crate) damage: Cell<Option<Bounds<ScaledPixels>>>,
}

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
            boxes: self.boxes.clone(),
            omitted: self.omitted.clone(),
            by_owner: self.by_owner.clone(),
        }
    }
}

impl SceneLayer {
    /// `(owner, index into boxes)` sorted, built at the first lookup: a layer
    /// no host repaints a box of never builds it.
    fn by_owner(&self) -> &[(u32, u32)] {
        self.by_owner.get_or_init(|| {
            let mut by_owner: Vec<(u32, u32)> =
                self.boxes.iter().enumerate().map(|(at, b)| (b.owner, at as u32)).collect();
            by_owner.sort_unstable();
            by_owner
        })
    }

    /// The indices of `owner`'s runs in `boxes`, in paint order.
    pub(crate) fn runs_of(&self, owner: u32) -> impl Iterator<Item = usize> + '_ {
        let by_owner = self.by_owner();
        let from = by_owner.partition_point(|&(o, _)| o < owner);
        by_owner[from..]
            .iter()
            .take_while(move |&&(o, _)| o == owner)
            .map(|&(_, at)| at as usize)
    }

    /// Union of the primitives' bounds within their content masks; `None`
    /// when the layer draws nothing.
    pub fn bounds(&self) -> Option<Bounds<ScaledPixels>> {
        self.bounds
    }

    /// The number of bindings and of box runs recorded.
    pub(crate) fn recorded_sizes(&self) -> (usize, usize) {
        (self.bindings.len(), self.boxes.len())
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
}

impl LayerHandle {
    pub(crate) fn new(layer: SceneLayer) -> Self {
        Self(Rc::new(LayerCell { content: RefCell::new(Rc::new(layer)), damage: Cell::new(None) }))
    }

    /// The current content.
    pub fn content(&self) -> Rc<SceneLayer> {
        self.0.content.borrow().clone()
    }

    /// Whether two handles name the same layer.
    pub fn same_layer(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod order_tests;
#[cfg(test)]
mod rerecord_tests;
#[cfg(test)]
mod placed_tests;

#[cfg(test)]
mod omitted_tests;
