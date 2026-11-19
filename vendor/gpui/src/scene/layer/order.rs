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
//! commands before the run, as a paint from scratch does. A later rectangle
//! the change reaches takes the bucket that paint gives it: above a moved
//! rectangle it meets that went higher, or, held by one that went or went
//! lower, one above the highest it meets before it; the others keep theirs
//! (`scene::layer::reorder` moves them). The layer keeps the orders of that
//! paint, so each later run is planned from exact orders again.

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
    /// Whether an ink or order of the run, or a later order, changed.
    pub(crate) changed: bool,
    /// `(command, bucket)`: later commands (primitives outside layers and
    /// layers) a paint from scratch gives another bucket, in paint order.
    pub(crate) later: Vec<(usize, DrawOrder)>,
}

/// A rectangle of the tree whose bucket the new run moves: `was` before,
/// `now` after (`None`: not there).
struct Moved {
    rect: Bounds<ScaledPixels>,
    was: Option<DrawOrder>,
    now: Option<DrawOrder>,
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
            // KaminIDE patch: a dormant primitive draws nothing.
            PaintOperation::Dormant(_) => {}
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
        // The rectangles whose bucket moved: the old run's (gone), the new
        // run's, and each later one the change moves, in paint order.
        let mut moved: Vec<Moved> = old.iter().map(|&(rect, t)| Moved { rect, was: Some(t), now: None }).collect();
        moved.extend(rects.iter().zip(&buckets).map(|(&rect, &t)| Moved { rect, was: None, now: Some(t) }));
        let mut later: Vec<(usize, DrawOrder)> = Vec::new();
        let mut layers_moved = false;
        tree_rects(ops, |at, r, x| {
            if at < run.end {
                return;
            }
            let meeting = moved.iter().filter(|m| m.rect.intersects(&r));
            let up = meeting.clone().filter_map(|m| m.now).max().unwrap_or(0);
            // Held by a rectangle that went or went down: it may go down too.
            let held = meeting.clone().any(|m| m.was == Some(x - 1) && m.now.is_none_or(|n| n + 1 < x));
            let now = if up + 1 > x {
                up + 1
            } else if held && up + 1 < x {
                self.highest_below(at, &run, &later, &moved, &r) + 1
            } else {
                x
            };
            if now != x {
                layers_moved |= matches!(ops[at], PaintOperation::Layer(_));
                moved.push(Moved { rect: r, was: Some(x), now: Some(now) });
                later.push((at, now));
            }
        });
        if layers_moved {
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
        let changed = !later.is_empty()
            || old.len() != rects.len()
            || old.iter().zip(rects.iter().zip(&buckets)).any(|((o, t), (n, u))| o != n || t != u);
        let ink = rects.iter().copied().reduce(|a, b| a.union(&b));
        Some(Plan { ink, orders, before, changed, later })
    }

    /// The highest bucket meeting `r` of the rectangles before command `at`:
    /// those of the commands outside `run` (with the buckets `later` gave)
    /// and of the new run.
    fn highest_below(
        &self,
        at: usize,
        run: &std::ops::Range<usize>,
        later: &[(usize, DrawOrder)],
        moved: &[Moved],
        r: &Bounds<ScaledPixels>,
    ) -> DrawOrder {
        let mut high = moved
            .iter()
            .filter(|m| m.was.is_none() && m.rect.intersects(r))
            .filter_map(|m| m.now)
            .max()
            .unwrap_or(0);
        tree_rects(&self.scene.paint_operations[..at], |i, n, t| {
            if !run.contains(&i) && n.intersects(r) {
                let t = later.binary_search_by_key(&i, |l| l.0).map_or(t, |k| later[k].1);
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
