//! KaminIDE patch: a box's run of commands replaced in a scene layer.
//!
//! The new run (a scratch scene the box painted with its key,
//! `Window::rerecord_layer_box`) goes where the old one was: the bucket of
//! the key in every sorted primitive list and the box's range of paint
//! commands. Nothing of another box is painted again or copied; quad
//! bindings and command ranges after the run move by its change in length.
//! The run may cover more than the area its key was taken for only when no
//! command of another box over the new ink is ordered against paint order.

use super::boxes::{BoxRange, ink_of};
use super::{Binding, SceneLayer};
use crate::scene::{DrawOrder, ORDER_STEP, PaintOperation, Primitive, Scene};
use crate::{Bounds, ScaledPixels};

/// The bucket of `key` in a list sorted by order.
fn bucket<T>(list: &[T], key: DrawOrder, order: impl Fn(&T) -> DrawOrder) -> std::ops::Range<usize> {
    let start = list.partition_point(|x| order(x) < key);
    let end = list.partition_point(|x| order(x) < key + ORDER_STEP);
    start..end
}

fn splice<T>(list: &mut Vec<T>, key: DrawOrder, order: impl Fn(&T) -> DrawOrder, new: Vec<T>) -> (usize, usize, usize) {
    let range = bucket(list, key, order);
    let (start, old) = (range.start, range.len());
    let len = new.len();
    list.splice(range, new);
    (start, old, len)
}

fn primitive_order(p: &Primitive) -> DrawOrder {
    match p {
        Primitive::Shadow(x) => x.order,
        Primitive::Quad(x) => x.order,
        Primitive::Path(x) => x.order,
        Primitive::Underline(x) => x.order,
        Primitive::MonochromeSprite(x) => x.order,
        Primitive::SubpixelSprite(x) => x.order,
        Primitive::PolychromeSprite(x) => x.order,
        Primitive::Surface(x) => x.order,
    }
}

impl SceneLayer {
    /// The new ink of box `at` if `new` (its run, painted with its key) can
    /// replace its run; `None` to refuse.
    pub(crate) fn check_box(&self, at: usize, new: &Scene) -> Option<Option<Bounds<ScaledPixels>>> {
        let b = self.boxes.get(at)?;
        if !b.replaceable || !new.paths.is_empty() || !new.surfaces.is_empty() || !new.groups.is_empty() {
            return None;
        }
        let mut ink: Option<Bounds<ScaledPixels>> = None;
        for operation in &new.paint_operations {
            let PaintOperation::Primitive(p) = operation else { return None };
            let rect = ink_of(p);
            if !rect.is_empty() {
                ink = Some(ink.map_or(rect, |i| i.union(&rect)));
            }
        }
        if new.paint_operations.len() as DrawOrder >= ORDER_STEP {
            return None;
        }
        let Some(rect) = ink else { return Some(None) };
        // The frame ordered the layer by its bounds.
        if !self.bounds.is_some_and(|l| l.intersect(&rect) == rect) {
            return None;
        }
        if b.area.intersect(&rect) != rect && !self.ordered_around(b, rect) {
            return None;
        }
        Some(Some(rect))
    }

    /// Whether every command of another box over `rect` keeps its paint
    /// order against box `b`'s key: earlier ones below it, later ones above.
    fn ordered_around(&self, b: &BoxRange, rect: Bounds<ScaledPixels>) -> bool {
        let (start, end) = (b.ops.start as usize, b.ops.end as usize);
        self.scene.paint_operations.iter().enumerate().all(|(i, operation)| {
            if (start..end).contains(&i) {
                return true;
            }
            let PaintOperation::Primitive(p) = operation else { return true };
            if ink_of(p).intersect(&rect).is_empty() {
                return true;
            }
            let order = primitive_order(p);
            if i < start { order < b.key } else { order >= b.key + ORDER_STEP }
        })
    }

    /// Replace the run of box `at` with `new` and its `bindings` (indices
    /// into `new`), after `check_box` gave `ink`; the damage, and whether
    /// the paint commands after the run moved.
    pub(crate) fn replace_box(
        &mut self,
        at: usize,
        new: Scene,
        bindings: Vec<Binding>,
        ink: Option<Bounds<ScaledPixels>>,
    ) -> (Option<Bounds<ScaledPixels>>, bool) {
        let b = self.boxes[at].clone();
        let key = b.key;
        let s = &mut self.scene;
        splice(&mut s.shadows, key, |x| x.order, new.shadows);
        let (quad_start, quad_old, quad_new) = splice(&mut s.quads, key, |x| x.order, new.quads);
        splice(&mut s.underlines, key, |x| x.order, new.underlines);
        splice(&mut s.monochrome_sprites, key, |x| x.order, new.monochrome_sprites);
        splice(&mut s.subpixel_sprites, key, |x| x.order, new.subpixel_sprites);
        splice(&mut s.polychrome_sprites, key, |x| x.order, new.polychrome_sprites);
        let (op_start, op_end) = (b.ops.start, b.ops.end);
        let op_new = new.paint_operations.len() as u32;
        s.paint_operations.splice(op_start as usize..op_end as usize, new.paint_operations);
        let quad_end = (quad_start + quad_old) as u32;
        let quad_shift = quad_new as i64 - quad_old as i64;
        let op_shift = op_new as i64 - (op_end - op_start) as i64;
        let moved = |index: u32, end: u32, shift: i64| if index >= end { (index as i64 + shift) as u32 } else { index };
        self.bindings.retain(|x| !(quad_start as u32..quad_end).contains(&x.quad));
        for x in &mut self.bindings {
            x.quad = moved(x.quad, quad_end, quad_shift);
            if x.op != u32::MAX {
                x.op = moved(x.op, op_end, op_shift);
            }
        }
        self.bindings.extend(bindings.into_iter().map(|x| Binding {
            quad: quad_start as u32 + x.quad,
            op: op_start + x.op,
            ..x
        }));
        self.bindings.sort_by_key(|x| (x.owner, x.value, x.quad));
        // Boxes are kept in paint order: the later ones' commands follow.
        let shift = |index: u32| (index as i64 + op_shift) as u32;
        for other in &mut self.boxes[at + 1..] {
            other.ops = shift(other.ops.start)..shift(other.ops.end);
        }
        let damage = match (b.ink, ink) {
            (Some(a), Some(n)) => Some(a.union(&n)),
            (a, n) => a.or(n),
        };
        let this = &mut self.boxes[at];
        this.ink = ink;
        this.ops = op_start..op_start + op_new;
        if op_shift != 0 {
            // Ranges copied from this scene index its commands.
            self.scene.blocks.clear();
        }
        self.version += 1;
        (damage, op_shift != 0)
    }
}
