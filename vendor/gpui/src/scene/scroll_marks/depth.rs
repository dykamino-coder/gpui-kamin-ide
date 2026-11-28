//! KaminIDE patch: retained depth metadata reorders translated paint without element painting.

use super::*;

impl Scene {
    pub(super) fn has_depth(&self, range: &Range<usize>) -> bool {
        self.paint_operations[range.clone()].iter().any(|op| matches!(op, PaintOperation::StartDepthContext))
    }

    /// Recompute draw orders from retained plane metadata after translating
    /// geometry. A dormant primitive entering the clip joins the same plane
    /// as a fresh insert; no layout, text shaping or element paint runs here.
    pub(super) fn replay_depth_scrolled(
        &mut self,
        range: Range<usize>,
        prev: &Scene,
        node: &dyn Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
        edges: &mut [(usize, usize)],
    ) {
        let mut marks = prev.scroll.within(&range).iter().peekable();
        let mut edge = 0;
        let mut at = range.start;
        while at < range.end {
            while edge < edges.len() && edges[edge].0 <= at {
                edges[edge].1 = self.paint_operations.len();
                edge += 1;
            }
            let op = &prev.paint_operations[at];
            let mark = marks.next_if(|m| m.op as usize == at);
            let open = self.scroll.open;
            let pivot = self.scroll.pivot.take();
            match (op, mark) {
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), Some(mark)) => {
                    let item = Item::of(op, mark);
                    let Piece::Primitive(mut primitive) = item.piece else { unreachable!() };
                    let (shift, chain) = node(mark.node);
                    self.scroll.pivot = record::geometry::scrolled(&mut primitive, shift, item.pivot.as_deref()).map(std::rc::Rc::new);
                    primitive.content_mask_mut().bounds = record::geometry::bounds(mark.own, shift);
                    self.scroll.open = Some((mark.node, chain));
                    self.copy_image(&primitive, &prev.book.images);
                    self.replay_operation(self.relabel_owned(PaintOperation::Primitive(primitive)));
                }
                (PaintOperation::StartLayer(bounds, _), Some(mark)) => {
                    let (shift, chain) = node(mark.node);
                    self.scroll.open = Some((mark.node, chain));
                    self.push_layer(record::geometry::bounds(*bounds, shift));
                }
                (PaintOperation::Reservoir(reservoir), None) => {
                    // Flat runs elsewhere in the same frame keep their spatial
                    // index; they cannot hold any depth metadata themselves.
                    let end = at + 1 + reservoir.drawn as usize;
                    self.replay_scrolled(at..end, prev, node, &mut []);
                    at = end - 1;
                }
                _ => self.replay_image_operation(op.clone(), &prev.book.images),
            }
            self.scroll.open = open;
            self.scroll.pivot = pivot;
            at += 1;
        }
        for edge in &mut edges[edge..] {
            edge.1 = self.paint_operations.len();
        }
    }
}
