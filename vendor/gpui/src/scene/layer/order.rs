//! KaminIDE patch: the draw orders of a box's run recorded again in a scene
//! layer.
//!
//! A layer's scene holds the orders a scene without layers gives the same
//! commands (`scene::layer::boxes`): a primitive outside layers takes
//! `ORDER_STEP` times one more than the highest bucket of the earlier
//! rectangles of the bounds tree it meets, a layer's primitives the layer's.
//! The commands give the tree's rectangles: primitives outside layers by their
//! ink, layers by their bounds at any depth (`Scene::push_layer`), retained
//! layers outside layers by theirs; the bottom layer adds none. A box's new
//! run takes, command by command, the bucket the tree gives it after the
//! commands before the run, as a paint from scratch does. The run is refused
//! when it changes the bucket of a later rectangle, which a paint from
//! scratch would then give: an accepted run keeps the layer's orders those of
//! that paint, so each later run is planned from exact orders again.

use super::SceneLayer;
use super::boxes::{BoxRange, ink_of};
use crate::scene::{DrawOrder, ORDER_STEP, PaintOperation, Primitive};
use crate::{Bounds, ScaledPixels};

/// The orders of a box's new run and what its splice needs.
pub(crate) struct Plan {
    /// The new run's ink.
    pub(crate) ink: Option<Bounds<ScaledPixels>>,
    /// The order of each new command.
    pub(crate) orders: Vec<DrawOrder>,
    /// `(kind, order, n)`: the `n` primitives of a kind and order of the old
    /// or new run painted before the run (each sorted list holds an order's
    /// entries in paint order).
    before: Vec<(u8, DrawOrder, usize)>,
    /// Whether an ink or order of the run changed.
    pub(crate) changed: bool,
}

impl Plan {
    /// The entries of `kind` (shadow 0, quad 1, underline 2) and `order`
    /// painted before the run.
    pub(crate) fn before(&self, kind: u8, order: DrawOrder) -> usize {
        self.before
            .iter()
            .find(|b| b.0 == kind && b.1 == order)
            .map_or(0, |b| b.2)
    }
}

/// The kind of a plain primitive, as `Plan::before` counts it.
pub(crate) fn plain_kind(p: &Primitive) -> Option<u8> {
    match p {
        Primitive::Shadow(_) => Some(0),
        Primitive::Quad(_) => Some(1),
        Primitive::Underline(_) => Some(2),
        _ => None,
    }
}

/// Each rectangle the bounds tree holds for `ops`, with its command and
/// bucket, in insertion order; `false` for 3D commands, whose depth sort
/// reorders them.
fn tree_rects(ops: &[PaintOperation], mut f: impl FnMut(usize, Bounds<ScaledPixels>, DrawOrder)) -> bool {
    let mut depth = 0usize;
    for (at, op) in ops.iter().enumerate() {
        match op {
            PaintOperation::Primitive(p) => {
                let ink = ink_of(p);
                if depth == 0 && !ink.is_empty() {
                    f(at, ink, p.order() / ORDER_STEP);
                }
            }
            PaintOperation::StartLayer(bounds, order) => {
                depth += 1;
                f(at, *bounds, order / ORDER_STEP);
            }
            PaintOperation::StartBottomLayer => depth += 1,
            PaintOperation::EndLayer => depth = depth.saturating_sub(1),
            PaintOperation::Layer(layer) => {
                if depth == 0 {
                    f(at, layer.bounds, layer.order / ORDER_STEP);
                }
            }
            PaintOperation::StartDepthContext
            | PaintOperation::EndDepthContext
            | PaintOperation::StartDepthPlane(_)
            | PaintOperation::EndDepthPlane => return false,
        }
    }
    true
}

fn max_meeting(rects: &[Bounds<ScaledPixels>], buckets: &[DrawOrder], r: &Bounds<ScaledPixels>) -> DrawOrder {
    rects
        .iter()
        .zip(buckets)
        .filter(|(n, _)| n.intersects(r))
        .map(|(_, t)| *t)
        .max()
        .unwrap_or(0)
}

impl SceneLayer {
    /// The orders of `new` (plain primitives, painted by box `b` again) as a
    /// paint from scratch gives them; `None` when that paint would order a
    /// later command otherwise, or the layer holds 3D commands.
    pub(crate) fn plan_box(&self, b: &BoxRange, new: &[PaintOperation]) -> Option<Plan> {
        let ops = &self.scene.paint_operations;
        let run = b.ops.start as usize..b.ops.end as usize;
        let mut rects = Vec::with_capacity(new.len());
        for op in new {
            let PaintOperation::Primitive(p) = op else { return None };
            let ink = ink_of(p);
            if ink.is_empty() {
                return None;
            }
            rects.push(ink);
        }
        let mut old = Vec::with_capacity(run.len());
        for op in &ops[run.clone()] {
            if let PaintOperation::Primitive(p) = op {
                old.push((ink_of(p), p.order() / ORDER_STEP));
            }
        }
        // The highest bucket below each new command, then its own.
        let mut below = vec![0; rects.len()];
        let exact = tree_rects(ops, |at, r, t| {
            if at < run.start {
                for (low, n) in below.iter_mut().zip(&rects) {
                    if n.intersects(&r) {
                        *low = (*low).max(t);
                    }
                }
            }
        });
        if !exact {
            return None;
        }
        let mut buckets: Vec<DrawOrder> = Vec::with_capacity(rects.len());
        for (j, n) in rects.iter().enumerate() {
            let inside = max_meeting(&rects[..j], &buckets, n);
            buckets.push(below[j].max(inside) + 1);
        }
        // Every later rectangle keeps its bucket.
        let mut kept = true;
        tree_rects(ops, |at, r, x| {
            if !kept || at < run.end {
                return;
            }
            let over = max_meeting(&rects, &buckets, &r);
            if over >= x {
                kept = false;
            } else if over + 1 < x
                && old.iter().any(|(o, t)| t + 1 == x && o.intersects(&r))
                && self.highest_below(at, &run, &r).max(over) + 1 < x
            {
                kept = false;
            }
        });
        if !kept {
            return None;
        }
        let orders: Vec<DrawOrder> = buckets.iter().map(|t| t * ORDER_STEP).collect();
        let mut before: Vec<(u8, DrawOrder, usize)> = Vec::new();
        let olds = ops[run.clone()].iter();
        let news = new.iter().zip(&orders).map(|(op, o)| (op, *o));
        for (op, order) in olds.map(|op| (op, order_of(op))).chain(news) {
            if let PaintOperation::Primitive(p) = op
                && let Some(kind) = plain_kind(p)
                && !before.iter().any(|b| b.0 == kind && b.1 == order)
            {
                before.push((kind, order, 0));
            }
        }
        for op in &ops[..run.start] {
            if let PaintOperation::Primitive(p) = op
                && let Some(kind) = plain_kind(p)
                && let Some(b) = before.iter_mut().find(|b| b.0 == kind && b.1 == p.order())
            {
                b.2 += 1;
            }
        }
        let changed = old.len() != rects.len()
            || old.iter().zip(rects.iter().zip(&buckets)).any(|((o, t), (n, u))| o != n || t != u);
        let ink = rects.iter().copied().reduce(|a, b| a.union(&b));
        Some(Plan { ink, orders, before, changed })
    }

    /// The highest bucket of the rectangles of commands before `at`, outside
    /// `run`, meeting `r`.
    fn highest_below(&self, at: usize, run: &std::ops::Range<usize>, r: &Bounds<ScaledPixels>) -> DrawOrder {
        let mut high = 0;
        tree_rects(&self.scene.paint_operations[..at], |i, n, t| {
            if !run.contains(&i) && n.intersects(r) {
                high = high.max(t);
            }
        });
        high
    }
}

fn order_of(op: &PaintOperation) -> DrawOrder {
    match op {
        PaintOperation::Primitive(p) => p.order(),
        _ => 0,
    }
}
