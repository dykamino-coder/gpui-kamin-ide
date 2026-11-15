//! KaminIDE patch: a box's run of commands replaced in a scene layer.
//!
//! The new run (a scratch scene the box painted with its key,
//! `Window::rerecord_layer_box`) goes where the old one was: the box's
//! entries in every sorted primitive list (boxes that do not overlap share a
//! key, see `Run`) and the box's range of paint commands. Nothing of another box is painted again or copied; quad
//! bindings and command ranges after the run move by its change in length.
//! The run may cover more than the area its key was taken for only when no
//! command of another box over the new ink is ordered against paint order.

use super::boxes::{BoxRange, ink_of};
use super::{Binding, SceneLayer};
use crate::scene::{DrawOrder, ORDER_STEP, PaintOperation, Primitive, Scene};
use crate::{Bounds, ScaledPixels};

/// The kinds of plain primitives a box's run may hold.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Shadow,
    Quad,
    Underline,
}

fn kind(operation: &PaintOperation) -> Option<Kind> {
    match operation {
        PaintOperation::Primitive(Primitive::Shadow(_)) => Some(Kind::Shadow),
        PaintOperation::Primitive(Primitive::Quad(_)) => Some(Kind::Quad),
        PaintOperation::Primitive(Primitive::Underline(_)) => Some(Kind::Underline),
        _ => None,
    }
}

/// The run of one box in one sorted list. The `n`th command of a box has
/// the order `key + 1 + n`, so each box sharing the key holds at most one
/// entry of an order, and the sort (stable) keeps those in paint order: the
/// box's entry of an order follows the `before[n]` entries of the boxes with
/// the key painted before it.
struct Run<'a> {
    key: DrawOrder,
    kind: Kind,
    before: &'a [[u32; 3]],
}

impl Run<'_> {
    fn at<T>(&self, list: &[T], order: impl Fn(&T) -> DrawOrder, n: usize) -> usize {
        let o = self.key + 1 + n as DrawOrder;
        list.partition_point(|x| order(x) < o) + self.before.get(n).map_or(0, |b| b[self.kind as usize]) as usize
    }

    /// Replace the entries of the commands `old` with those of `new` (both
    /// the box's commands in paint order, with `items` the entries of `new`
    /// of this kind); the indices removed (in removal order) and inserted.
    fn splice<T>(
        &self,
        list: &mut Vec<T>,
        order: impl Fn(&T) -> DrawOrder + Copy,
        old: &[PaintOperation],
        new: &[PaintOperation],
        items: Vec<T>,
    ) -> (Vec<usize>, Vec<usize>) {
        let mut removed = Vec::new();
        for (n, operation) in old.iter().enumerate().rev() {
            if kind(operation) == Some(self.kind) {
                let at = self.at(list, order, n);
                list.remove(at);
                removed.push(at);
            }
        }
        let mut items = items.into_iter();
        let mut inserted = Vec::new();
        for (n, operation) in new.iter().enumerate() {
            if kind(operation) == Some(self.kind)
                && let Some(item) = items.next()
            {
                let at = self.at(list, order, n);
                list.insert(at, item);
                inserted.push(at);
            }
        }
        (removed, inserted)
    }
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
        if !b.replaceable || !new.groups.is_empty() {
            return None;
        }
        let mut ink: Option<Bounds<ScaledPixels>> = None;
        for operation in &new.paint_operations {
            kind(operation)?;
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
        // Entries of each order and kind of the boxes with the key before this one.
        let longest = new.paint_operations.len().max((b.ops.end - b.ops.start) as usize);
        let mut before = vec![[0u32; 3]; longest];
        for other in self.boxes[..at].iter().filter(|o| o.key == key) {
            let ops = &self.scene.paint_operations[other.ops.start as usize..other.ops.end as usize];
            for (n, operation) in ops.iter().enumerate().take(longest) {
                if let Some(k) = kind(operation) {
                    before[n][k as usize] += 1;
                }
            }
        }
        let old: Vec<PaintOperation> = self.scene.paint_operations[b.ops.start as usize..b.ops.end as usize].to_vec();
        let s = &mut self.scene;
        let run = |kind| Run { key, kind, before: &before };
        run(Kind::Shadow).splice(&mut s.shadows, |x| x.order, &old, &new.paint_operations, new.shadows);
        let (quads_removed, quads_inserted) =
            run(Kind::Quad).splice(&mut s.quads, |x| x.order, &old, &new.paint_operations, new.quads);
        run(Kind::Underline).splice(&mut s.underlines, |x| x.order, &old, &new.paint_operations, new.underlines);
        let (op_start, op_end) = (b.ops.start, b.ops.end);
        let op_new = new.paint_operations.len() as u32;
        s.paint_operations.splice(op_start as usize..op_end as usize, new.paint_operations);
        let op_shift = op_new as i64 - (op_end - op_start) as i64;
        // Bindings follow their quads and commands; the box's old ones go.
        let gone = |x: &Binding| (op_start..op_end).contains(&x.op);
        let mut kept: Vec<Binding> = Vec::with_capacity(self.bindings.len());
        // Removal indices are taken one after another from the list as it
        // shrinks; a binding of a removed quad goes.
        let mut original = self.bindings.clone();
        for &at in &quads_removed {
            for x in &mut original {
                if x.quad != u32::MAX && x.quad as usize > at {
                    x.quad -= 1;
                } else if x.quad as usize == at {
                    x.quad = u32::MAX;
                }
            }
        }
        for (x, was) in original.into_iter().zip(&self.bindings) {
            if gone(was) || x.quad == u32::MAX {
                continue;
            }
            kept.push(x);
        }
        for &at in &quads_inserted {
            for x in &mut kept {
                if x.quad as usize >= at {
                    x.quad += 1;
                }
            }
        }
        for x in &mut kept {
            if x.op != u32::MAX && x.op >= op_end {
                x.op = (x.op as i64 + op_shift) as u32;
            }
        }
        kept.extend(bindings.into_iter().filter_map(|x| {
            Some(Binding { quad: *quads_inserted.get(x.quad as usize)? as u32, op: op_start + x.op, ..x })
        }));
        self.bindings = kept;        self.bindings.sort_by_key(|x| (x.owner, x.value, x.quad));
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
