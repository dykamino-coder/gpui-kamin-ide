//! KaminIDE patch: the paint ranges of boxes in a scene layer.
//!
//! While a layer is recorded, the own paint of a box (its shadows, fill,
//! borders and host painter, `Window::with_box_paint`) takes one draw-order
//! key from the scene's bounds tree, as one primitive covering the box and
//! its own ink would, and every command of it the next order above the key
//! in paint order. Boxes that do not overlap may share a key; each holds at
//! most one entry of an order of the key's bucket, in paint order among them,
//! and one run of paint commands, which a later recording of the box replaces
//! whole (`Window::rerecord_layer_box`), whatever the number of commands.
//! Blink keeps the display items of a client in paint order and repaints a
//! client's items into a new artifact (`paint_controller.cc:228`); the key
//! lets this layer splice the box's run in place instead.

use super::super::{
    BoundsTree, DrawOrder, ORDER_STEP, PaintOperation, Primitive, ScaledPixels, Scene,
    placed_bounds,
};
use crate::{Bounds, ContentMask, Pixels, TransformationMatrix};
use std::ops::Range;

/// The paint state a box's own paint ran in: what its later recording
/// restores.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxEnv {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) mask: ContentMask<Pixels>,
    pub(crate) transformation: TransformationMatrix,
    pub(crate) opacity: f32,
    pub(crate) rem_size: Pixels,
    /// Where the host placed the box's fill (Window::box_fill_bounds).
    pub(crate) fill: Option<Bounds<Pixels>>,
}

/// A box's own paint in a layer: its owner, key, the area its key was taken
/// for, its current ink, its run of paint commands and its environment.
#[derive(Clone, Debug)]
pub(crate) struct BoxRange {
    pub(crate) owner: u32,
    pub(crate) key: DrawOrder,
    pub(crate) area: Bounds<ScaledPixels>,
    pub(crate) ink: Option<Bounds<ScaledPixels>>,
    pub(crate) ops: Range<u32>,
    pub(crate) env: BoxEnv,
    /// The run is one bucket of plain primitives, which a recording replaces.
    pub(crate) replaceable: bool,
}

/// A box's own paint being recorded: where its commands start and, for a
/// later recording, the key it keeps.
pub(crate) struct OpenBox {
    owner: u32,
    ops: usize,
    area: Bounds<ScaledPixels>,
    env: BoxEnv,
    key: Option<DrawOrder>,
}

/// The clipped placed bounds of a primitive.
pub(crate) fn ink_of(primitive: &Primitive) -> Bounds<ScaledPixels> {
    placed_bounds(primitive).intersect(&primitive.content_mask().bounds)
}

fn union(a: Option<Bounds<ScaledPixels>>, b: Bounds<ScaledPixels>) -> Option<Bounds<ScaledPixels>> {
    if b.is_empty() {
        return a;
    }
    Some(a.map_or(b, |a| a.union(&b)))
}

impl Scene {
    /// Start the own paint of box `owner` with `area` (its placed bounds,
    /// device pixels); `key` keeps the key of an earlier recording. `false`
    /// where the scene orders by other means (stacking layers, 3D, a box
    /// already open): the box paints as before.
    pub(crate) fn open_box(
        &mut self,
        owner: u32,
        area: Bounds<ScaledPixels>,
        env: BoxEnv,
        key: Option<DrawOrder>,
    ) -> bool {
        if self.open_box.is_some()
            || !self.layer_stack.is_empty()
            || !self.depth.plane_stack.is_empty()
        {
            return false;
        }
        self.open_box = Some(OpenBox {
            owner,
            ops: self.paint_operations.len(),
            area,
            env,
            key,
        });
        true
    }

    /// Whether a box's own paint is open: its primitives wait for the key.
    pub(crate) fn box_open(&self) -> bool {
        self.open_box.is_some()
    }

    /// The fill bounds the open box's paint placed.
    pub(crate) fn open_box_fill(&self) -> Option<Bounds<Pixels>> {
        self.open_box.as_ref()?.env.fill
    }

    pub(crate) fn set_open_box_fill(&mut self, fill: Bounds<Pixels>) {
        if let Some(open) = self.open_box.as_mut() {
            open.env.fill = Some(fill);
        }
    }

    /// Finish the open box: one key for its area and ink, and the orders of
    /// its commands above the key in paint order.
    pub(crate) fn close_box(&mut self) -> Option<BoxRange> {
        let open = self.open_box.take()?;
        let ops = open.ops..self.paint_operations.len();
        let mut ink = None;
        let mut plain = true;
        for operation in &self.paint_operations[ops.clone()] {
            match operation {
                PaintOperation::Primitive(p) => {
                    // Sprites sort by tile within an order: not in paint order.
                    plain &= matches!(
                        p,
                        Primitive::Shadow(_) | Primitive::Quad(_) | Primitive::Underline(_)
                    );
                    ink = union(ink, ink_of(p));
                }
                _ => plain = false,
            }
        }
        let area = union(ink, open.area).unwrap_or(open.area);
        let key = match open.key {
            Some(key) => key,
            None => tree_insert(&mut self.primitive_bounds, area) * ORDER_STEP,
        };
        let count = (ops.end - ops.start) as u32;
        plain &= count < ORDER_STEP;
        self.order_box_commands(ops.clone(), key);
        Some(BoxRange {
            owner: open.owner,
            key,
            area,
            ink,
            ops: ops.start as u32..ops.end as u32,
            env: open.env,
            replaceable: plain,
        })
    }

    /// Give the primitives of the commands `ops` (all inserted last, in
    /// paint order) the orders `key + 1 + n`, in their lists and commands.
    fn order_box_commands(&mut self, ops: Range<usize>, key: DrawOrder) {
        let mut taken = [0usize; 8];
        let count = |p: &Primitive| match p {
            Primitive::Shadow(_) => 0,
            Primitive::Quad(_) => 1,
            Primitive::Path(_) => 2,
            Primitive::Underline(_) => 3,
            Primitive::MonochromeSprite(_) => 4,
            Primitive::SubpixelSprite(_) => 5,
            Primitive::PolychromeSprite(_) => 6,
            Primitive::Surface(_) => 7,
        };
        for operation in &self.paint_operations[ops.clone()] {
            if let PaintOperation::Primitive(p) = operation {
                taken[count(p)] += 1;
            }
        }
        let lens = [
            self.shadows.len(),
            self.quads.len(),
            self.paths.len(),
            self.underlines.len(),
            self.monochrome_sprites.len(),
            self.subpixel_sprites.len(),
            self.polychrome_sprites.len(),
            self.surfaces.len(),
        ];
        let mut next: [usize; 8] = std::array::from_fn(|k| lens[k].saturating_sub(taken[k]));
        for (n, at) in ops.enumerate() {
            let order = key + (1 + n as DrawOrder).min(ORDER_STEP - 1);
            let PaintOperation::Primitive(p) = &mut self.paint_operations[at] else {
                continue;
            };
            let kind = count(p);
            set_order(p, order);
            let index = next[kind];
            next[kind] += 1;
            match kind {
                0 => self.shadows[index].order = order,
                1 => self.quads[index].order = order,
                2 => self.paths[index].order = order,
                3 => self.underlines[index].order = order,
                4 => self.monochrome_sprites[index].order = order,
                5 => self.subpixel_sprites[index].order = order,
                6 => self.polychrome_sprites[index].order = order,
                _ => self.surfaces[index].order = order,
            }
        }
    }
}

fn tree_insert(
    tree: &mut std::rc::Rc<BoundsTree<ScaledPixels>>,
    area: Bounds<ScaledPixels>,
) -> DrawOrder {
    std::rc::Rc::make_mut(tree).insert(area)
}

fn set_order(p: &mut Primitive, order: DrawOrder) {
    match p {
        Primitive::Shadow(x) => x.order = order,
        Primitive::Quad(x) => x.order = order,
        Primitive::Path(x) => x.order = order,
        Primitive::Underline(x) => x.order = order,
        Primitive::MonochromeSprite(x) => x.order = order,
        Primitive::SubpixelSprite(x) => x.order = order,
        Primitive::PolychromeSprite(x) => x.order = order,
        Primitive::Surface(x) => x.order = order,
    }
}
