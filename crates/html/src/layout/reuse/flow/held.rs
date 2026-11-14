//! The step-8 wrapper (`PaintLast`) of a recorded root-flow child goes on
//! outside its records.
//!
//! A positioned block with `z-index: auto | 0` (and a box that makes a
//! stacking context of level zero) paints at step 8 of CSS 2.1 Appendix E:
//! its wrapper hands the paint to the positioned phase of the stacking
//! context. A recorder outside the wrapper would see an empty paint, so the
//! build leaves the wrapper to the flow (`deferred::hold_paint_last`), the
//! recorder wraps the bare block and the wrapper goes on last. A replay is
//! wrapped the same way with the recorded key (`FlowCx::replay`): the key
//! comes from the paint-key counter, which the record's start counters pin.
//! Blink keeps the same split: a self-painting layer's cached subsequence
//! is replayed where the layer paints in its stacking context
//! (`PaintLayerPainter::Paint`, paint_layer_painter.cc:412-424).

use super::{FlowCx, Mark};
use crate::dom::Element;
use crate::layout::cx::deferred::HeldPaintLast;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, IntoElement};

/// `child` in its step-8 wrapper with paint key `key`.
pub(super) fn paint_last(child: AnyElement, key: u64) -> AnyElement {
    crate::paint::coordinator::PaintLast::new(child)
        .key(key)
        .into_any_element()
}

impl FlowCx {
    /// Wrap the just built element of child `e` in records (`record_built`),
    /// then in the step-8 wrapper its build left to the flow.
    pub(crate) fn record(
        &self,
        e: &Element,
        edge: bool,
        mut mark: Mark,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let paint = &opts.cx.deferred.paint;
        let held = match paint.held_paint_last.replace(HeldPaintLast::None) {
            HeldPaintLast::Held(node, key, at) if node == e.node_id && at < out.len() => {
                Some((key, at))
            }
            _ => None,
        };
        mark.paint_last = held.map(|(key, _)| key);
        self.record_built(e, edge, mark, opts, out);
        if let Some((key, at)) = held {
            // The records replace the element in place.
            let child = out.remove(at);
            out.insert(at, paint_last(child, key));
        }
    }
}
