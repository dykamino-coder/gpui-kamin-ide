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
        nodes.into_iter().try_for_each(|node| self.node_check(&node).map_err(|(refusal, _)| refusal))
    }

    /// The command of the first node range (`nodes`) that refuses a scrolled
    /// replay and what it is, for the `why` journal: a page that builds its
    /// scroll steps names the paint that holds no node mark.
    pub(crate) fn scroll_refusal_detail(&self, nodes: impl Iterator<Item = Range<usize>>) -> Option<String> {
        nodes.into_iter().find_map(|node| {
            let (refusal, at) = self.node_check(&node).err()?;
            let what = match self.paint_operations.get(at) {
                Some(PaintOperation::Primitive(primitive)) => primitive_name(primitive, "primitive"),
                Some(PaintOperation::Dormant(primitive)) => primitive_name(primitive, "dormant"),
                Some(PaintOperation::StartLayer(bounds, _)) => format!("layer at {bounds:?}"),
                Some(PaintOperation::Layer(_)) => "retained layer".into(),
                Some(PaintOperation::Reservoir(_)) => "reservoir".into(),
                _ => "end or depth command".into(),
            };
            let marks = self.scroll.within(&node);
            let near = marks.iter().rev().find(|mark| (mark.op as usize) < at).map(|mark| (mark.op, mark.node));
            Some(format!(
                "{refusal:?}: command {at} ({what}) in the node range {node:?} of {} marks; last mark before it {near:?}",
                marks.len()
            ))
        })
    }

    fn node_check(&self, node: &Range<usize>) -> Result<(), (ScrollRefusal, usize)> {
        let mut marks = self.scroll.within(node).iter().peekable();
        // The node of each layer open in the range.
        let mut layers: Vec<ScrollNodeId> = Vec::new();
        let mut at = node.start;
        while at < node.end {
            let mark = marks.next_if(|mark| mark.op as usize == at);
            if mark.is_some_and(|mark| layers.last().is_some_and(|&layer| layer != mark.node)) {
                return Err((ScrollRefusal::ForeignMark, at));
            }
            match (&self.paint_operations[at], mark) {
                (PaintOperation::Reservoir(reservoir), None) if layers.is_empty() => at += reservoir.drawn as usize,
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), Some(_)) => {}
                (PaintOperation::StartLayer(..), Some(mark)) => layers.push(mark.node),
                (PaintOperation::EndLayer, None) if layers.pop().is_some() => {}
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), None) => {
                    return Err((ScrollRefusal::Unmarked, at));
                }
                (PaintOperation::StartLayer(..), None) => return Err((ScrollRefusal::UnmarkedLayer, at)),
                (PaintOperation::Layer(_), _) => return Err((ScrollRefusal::RetainedLayer, at)),
                (PaintOperation::Reservoir(_), _) => return Err((ScrollRefusal::LayerReservoir, at)),
                (PaintOperation::StartBottomLayer, _) => return Err((ScrollRefusal::BottomLayer, at)),
                (PaintOperation::EndLayer, _) => return Err((ScrollRefusal::UnbalancedLayer, at)),
                _ => return Err((ScrollRefusal::DepthSort, at)),
            }
            at += 1;
        }
        match (at == node.end, layers.is_empty()) {
            (false, _) => Err((ScrollRefusal::LayerReservoir, at)),
            (true, false) => Err((ScrollRefusal::UnbalancedLayer, at)),
            (true, true) => Ok(()),
        }
    }
}

fn primitive_name(primitive: &super::super::Primitive, as_: &str) -> String {
    use super::super::Primitive as P;
    let kind = match primitive {
        P::Shadow(_) => "shadow",
        P::Quad(_) => "quad",
        P::Gradient(_) => "gradient",
        P::Path(_) => "path",
        P::Underline(_) => "underline",
        P::MonochromeSprite(_) => "glyph",
        P::SubpixelSprite(_) => "subpixel glyph",
        P::PolychromeSprite(_) => "image",
        P::Surface(_) => "surface",
    };
    format!("{as_} {kind} at {:?}", primitive.bounds())
}
