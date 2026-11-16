//! KaminIDE patch: a box's run of commands replaced in a scene layer.
//!
//! The new run (a scratch scene the box painted, `Window::rerecord_layer_box`)
//! takes the orders a paint from scratch gives it (`scene::layer::order`) and
//! goes where the old one was: the box's entries in every sorted primitive
//! list and the box's range of paint commands. A sorted list holds the
//! entries of one order in paint order, so an entry's place is the first of
//! its order, after those painted before the run and the run's earlier ones.
//! Nothing of another box is painted again or copied; quad bindings and
//! command ranges after the run move by its change in length.

use super::order::{Plan, plain_kind};
use super::{Binding, SceneLayer, layer_counters};
use crate::scene::{DrawOrder, PaintOperation, Primitive, Scene};
use crate::{Bounds, ScaledPixels};
use std::sync::atomic::Ordering::Relaxed;

/// The order of `operation` if it is a primitive of `kind`.
fn order_in(operation: &PaintOperation, kind: u8) -> Option<DrawOrder> {
    match operation {
        PaintOperation::Primitive(p) if plain_kind(p) == Some(kind) => Some(p.order()),
        _ => None,
    }
}

/// Replace in `list` (the sorted entries of `kind`) the entries of the
/// commands `old` with those of `new` (both a box's run in paint order, the
/// new one with its orders); the indices removed and inserted as each was
/// taken, and the final index of each inserted entry.
fn splice<T>(
    list: &mut Vec<T>,
    order: impl Fn(&T) -> DrawOrder,
    kind: u8,
    entry: impl Fn(&Primitive) -> Option<T>,
    (old, new): (&[PaintOperation], &[PaintOperation]),
    plan: &Plan,
) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let place = |list: &Vec<T>, run: &[PaintOperation], n: usize, o: DrawOrder| {
        let earlier = run[..n].iter().filter(|x| order_in(x, kind) == Some(o)).count();
        list.partition_point(|x| order(x) < o) + plan.before(kind, o) + earlier
    };
    let mut removed = Vec::new();
    for (n, operation) in old.iter().enumerate().rev() {
        if let Some(o) = order_in(operation, kind) {
            let at = place(list, old, n, o);
            list.remove(at);
            removed.push(at);
        }
    }
    let (mut inserted, mut last) = (Vec::new(), Vec::new());
    for (n, operation) in new.iter().enumerate() {
        let PaintOperation::Primitive(p) = operation else { continue };
        let (Some(o), Some(item)) = (order_in(operation, kind), entry(p)) else { continue };
        let at = place(list, new, n, o);
        list.insert(at, item);
        for x in &mut last {
            if *x >= at {
                *x += 1;
            }
        }
        inserted.push(at);
        last.push(at);
    }
    (removed, inserted, last)
}

impl SceneLayer {
    /// The plan of `new` (box `at`'s run painted again) if it can replace
    /// the run; `None` to refuse.
    pub(crate) fn check_box(&self, at: usize, new: &Scene) -> Option<Plan> {
        let b = self.boxes.get(at)?;
        if !b.replaceable || !new.groups.is_empty() {
            return None;
        }
        let plain = |op: &PaintOperation| matches!(op, PaintOperation::Primitive(p) if plain_kind(p).is_some());
        if !new.paint_operations.iter().all(plain) {
            return None;
        }
        let plan = self
            .plan_box(b, &new.paint_operations)
            .filter(|plan| plan.later.is_empty() || self.lists_follow_commands());
        let Some(plan) = plan else {
            layer_counters::RUNS_REORDERED.fetch_add(1, Relaxed);
            return None;
        };
        // The frame ordered the layer by its bounds.
        if let Some(rect) = plan.ink
            && !self.bounds.is_some_and(|l| l.intersect(&rect) == rect)
        {
            return None;
        }
        Some(plan)
    }

    /// Replace the run of box `at` with `new` and its `bindings` (indices
    /// into `new`) as `check_box` planned; the damage, and whether the paint
    /// commands after the run moved.
    pub(crate) fn replace_box(
        &mut self,
        at: usize,
        new: Scene,
        bindings: Vec<Binding>,
        plan: Plan,
    ) -> (Option<Bounds<ScaledPixels>>, bool) {
        let b = self.boxes[at].clone();
        let (op_start, op_end) = (b.ops.start, b.ops.end);
        let old: Vec<PaintOperation> =
            self.scene.paint_operations[op_start as usize..op_end as usize].to_vec();
        let mut fresh = new.paint_operations;
        for (operation, &order) in fresh.iter_mut().zip(&plan.orders) {
            if let PaintOperation::Primitive(p) = operation {
                *p.order_mut() = order;
            }
        }
        let s = &mut self.scene;
        let runs = (old.as_slice(), fresh.as_slice());
        let shadow = |p: &Primitive| match p {
            Primitive::Shadow(x) => Some(*x),
            _ => None,
        };
        let quad = |p: &Primitive| match p {
            Primitive::Quad(x) => Some(*x),
            _ => None,
        };
        let underline = |p: &Primitive| match p {
            Primitive::Underline(x) => Some(*x),
            _ => None,
        };
        splice(&mut s.shadows, |x| x.order, 0, shadow, runs, &plan);
        let (quads_removed, quads_inserted, quads_at) = splice(&mut s.quads, |x| x.order, 1, quad, runs, &plan);
        splice(&mut s.underlines, |x| x.order, 2, underline, runs, &plan);
        let op_new = fresh.len() as u32;
        s.paint_operations
            .splice(op_start as usize..op_end as usize, fresh);
        let op_shift = op_new as i64 - (op_end - op_start) as i64;
        self.bindings = self.moved_bindings(b.ops.clone(), op_shift, &quads_removed, &quads_inserted);
        self.bindings.extend(bindings.into_iter().filter_map(|x| {
            Some(Binding {
                quad: *quads_at.get(x.quad as usize)? as u32,
                op: op_start + x.op,
                ..x
            })
        }));
        self.bindings.sort_by_key(|x| (x.owner, x.value, x.quad));
        // Boxes are kept in paint order: the later ones' commands follow.
        let shift = |index: u32| (index as i64 + op_shift) as u32;
        for other in &mut self.boxes[at + 1..] {
            other.ops = shift(other.ops.start)..shift(other.ops.end);
        }
        let mut damage = match (b.ink, plan.ink) {
            (Some(a), Some(n)) => Some(a.union(&n)),
            (a, n) => a.or(n),
        };
        if !plan.later.is_empty() {
            let later: Vec<(usize, DrawOrder)> =
                plan.later.iter().map(|&(i, t)| (shift(i as u32) as usize, t)).collect();
            if let Some(ink) = self.move_later(&later) {
                damage = Some(damage.map_or(ink, |d| d.union(&ink)));
            }
            layer_counters::RUNS_MOVED_LATER.fetch_add(1, Relaxed);
        }
        let this = &mut self.boxes[at];
        this.ink = plan.ink;
        this.ops = op_start..op_start + op_new;
        if op_shift != 0 || plan.changed {
            // Ranges copied from this scene index its commands and orders.
            self.scene.blocks.clear();
        }
        self.version += 1;
        (damage, op_shift != 0)
    }

    /// The bindings of other runs after quads were removed and inserted (each
    /// index as it was taken) and the commands after `run` moved by `shift`.
    fn moved_bindings(
        &self,
        run: std::ops::Range<u32>,
        shift: i64,
        removed: &[usize],
        inserted: &[usize],
    ) -> Vec<Binding> {
        let mut original = self.bindings.clone();
        for &at in removed {
            for x in &mut original {
                if x.quad != u32::MAX && x.quad as usize > at {
                    x.quad -= 1;
                } else if x.quad as usize == at {
                    x.quad = u32::MAX;
                }
            }
        }
        let mut kept: Vec<Binding> = original
            .into_iter()
            .zip(&self.bindings)
            .filter(|(x, was)| !run.contains(&was.op) && x.quad != u32::MAX)
            .map(|(x, _)| x)
            .collect();
        for &at in inserted {
            for x in &mut kept {
                if x.quad as usize >= at {
                    x.quad += 1;
                }
            }
        }
        for x in &mut kept {
            if x.op != u32::MAX && x.op >= run.end {
                x.op = (x.op as i64 + shift) as u32;
            }
        }
        kept
    }
}
