//! KaminIDE patch: whether a range of a finished scene can be replayed
//! scrolled, and if not, the first command that refuses it, so a page that
//! builds every scroll step names what it painted (`ScrollRefusal`).

use super::super::{PaintOperation, Scene};
use crate::{ScrollNodeId, ScrollRefusal};
use std::ops::Range;

impl Scene {
    /// Whether `range` of this finished scene can be replayed scrolled
    /// (`scroll_check`).
    pub(crate) fn scrollable(&self, range: &Range<usize>, nodes: impl Iterator<Item = Range<usize>>) -> bool {
        self.scroll_check(range, nodes).is_ok()
    }

    /// Whether `range` can be replayed scrolled: its commands keep their
    /// orders under one shift (no bottom layer or depth sorting, balanced
    /// layers), and inside each node's scene range (`nodes`) every command is
    /// a marked primitive, a marked layer of one node holding marked
    /// primitives of it, or a reservoir with what it drew. Otherwise the
    /// first command that refuses.
    pub(crate) fn scroll_check(
        &self,
        range: &Range<usize>,
        nodes: impl Iterator<Item = Range<usize>>,
    ) -> Result<(), ScrollRefusal> {
        let mut layers = 0usize;
        for operation in &self.paint_operations[range.clone()] {
            match operation {
                PaintOperation::StartLayer(..) => layers += 1,
                PaintOperation::EndLayer => layers = layers.checked_sub(1).ok_or(ScrollRefusal::UnbalancedLayer)?,
                PaintOperation::StartBottomLayer => return Err(ScrollRefusal::BottomLayer),
                PaintOperation::StartDepthContext
                | PaintOperation::EndDepthContext
                | PaintOperation::StartDepthPlane(_)
                | PaintOperation::EndDepthPlane => return Err(ScrollRefusal::DepthSort),
                _ => {}
            }
        }
        if layers != 0 {
            return Err(ScrollRefusal::UnbalancedLayer);
        }
        nodes.into_iter().try_for_each(|node| self.node_check(&node))
    }

    fn node_check(&self, node: &Range<usize>) -> Result<(), ScrollRefusal> {
        let mut marks = self.scroll.within(node).iter().peekable();
        // The node of each layer open in the range.
        let mut layers: Vec<ScrollNodeId> = Vec::new();
        let mut at = node.start;
        while at < node.end {
            let mark = marks.next_if(|mark| mark.op as usize == at);
            if mark.is_some_and(|mark| layers.last().is_some_and(|&layer| layer != mark.node)) {
                return Err(ScrollRefusal::ForeignMark);
            }
            match (&self.paint_operations[at], mark) {
                (PaintOperation::Reservoir(reservoir), None) if layers.is_empty() => at += reservoir.drawn as usize,
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), Some(_)) => {}
                (PaintOperation::StartLayer(..), Some(mark)) => layers.push(mark.node),
                (PaintOperation::EndLayer, None) if layers.pop().is_some() => {}
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), None) => {
                    return Err(ScrollRefusal::Unmarked);
                }
                (PaintOperation::StartLayer(..), None) => return Err(ScrollRefusal::UnmarkedLayer),
                (PaintOperation::Layer(_), _) => return Err(ScrollRefusal::RetainedLayer),
                (PaintOperation::Reservoir(_), _) => return Err(ScrollRefusal::LayerReservoir),
                (PaintOperation::StartBottomLayer, _) => return Err(ScrollRefusal::BottomLayer),
                (PaintOperation::EndLayer, _) => return Err(ScrollRefusal::UnbalancedLayer),
                _ => return Err(ScrollRefusal::DepthSort),
            }
            at += 1;
        }
        match (at == node.end, layers.is_empty()) {
            (false, _) => Err(ScrollRefusal::LayerReservoir),
            (true, false) => Err(ScrollRefusal::UnbalancedLayer),
            (true, true) => Ok(()),
        }
    }
}
