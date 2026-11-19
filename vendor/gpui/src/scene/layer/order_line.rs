//! KaminIDE patch: the draw orders of a run recorded again inside a stacking
//! layer (the piece of an inline box's band on its line,
//! `text_system::band_piece`).
//!
//! Every primitive inside a stacking layer takes the layer's order
//! (`Scene::insert_primitive`), and the layer's rectangle in the bounds tree
//! is its bounds (`Scene::push_layer`), not the ink of what it holds. A run
//! painted again inside the layer therefore takes the layer's order for each
//! command and moves no rectangle of the tree: nothing after it is ordered
//! otherwise. The layer's order is read from the commands, the stacking
//! layer that encloses the run, so a run placed from a paint record
//! (`super::placed`) takes the order its placement gave.

use super::SceneLayer;
use super::boxes::{BoxRange, ink_of};
use super::order::{Plan, before_run};
use crate::scene::{DrawOrder, PaintOperation};

/// The order of the stacking layer around command `at` of `ops`; `None`
/// outside layers or in the bottom layer, whose primitives take orders of
/// their own.
fn enclosing_layer(ops: &[PaintOperation], at: usize) -> Option<DrawOrder> {
    let mut depth = 0usize;
    for op in ops[..at].iter().rev() {
        match op {
            PaintOperation::EndLayer => depth += 1,
            PaintOperation::StartLayer(_, order) if depth == 0 => return Some(*order),
            PaintOperation::StartBottomLayer if depth == 0 => return None,
            PaintOperation::StartLayer(..) | PaintOperation::StartBottomLayer => depth -= 1,
            _ => {}
        }
    }
    None
}

impl SceneLayer {
    /// The orders of `new` (plain primitives, the run `b` inside a stacking
    /// layer painted again) as a paint from scratch gives them: the layer's
    /// order for each.
    pub(crate) fn plan_layer_run(&self, b: &BoxRange, new: &[PaintOperation]) -> Option<Plan> {
        let ops = &self.scene.paint_operations;
        let run = b.ops.start as usize..b.ops.end as usize;
        let order = enclosing_layer(ops, run.start)?;
        let mut inks = Vec::with_capacity(new.len());
        for op in new {
            let PaintOperation::Primitive(p) = op else { return None };
            let ink = ink_of(p);
            if ink.is_empty() {
                return None;
            }
            inks.push(ink);
        }
        let old: Vec<_> = ops[run.clone()]
            .iter()
            .filter_map(|op| match op {
                PaintOperation::Primitive(p) => Some(ink_of(p)),
                _ => None,
            })
            .collect();
        let orders = vec![order; new.len()];
        let before = before_run(ops, &run, new, &orders);
        let changed = old != inks;
        let ink = inks.iter().copied().reduce(|a, b| a.union(&b));
        Some(Plan { ink, orders, before, changed, later: Vec::new() })
    }
}
