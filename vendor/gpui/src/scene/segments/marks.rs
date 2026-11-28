//! KaminIDE patch: the scroll marks of a recorded flow in its frozen segments.
//!
//! A node opened while a flow records (`Window::record_paint_segments`)
//! marks its commands in the draft; each segment copies them moved to its
//! place. A marked command is inserted there under its node again, moved
//! and clipped, so the segment's copy is marked (or dormant) as a paint at
//! its place would be, and the segment relays where it put the draft's
//! commands (`ScrollBook::relayed`): the landing that shows the node in a
//! frame moves the node's ranges and masks through it.

use super::*;
use crate::scene::scroll_marks::{ScrollMark, carry::Carry};

impl Scene {
    /// Insert `primitive`, the copy of a command marked `mark` (dormant or
    /// not), moved by `offset` and clipped by `clip`, under its node.
    pub(super) fn insert_marked(
        &mut self,
        mut primitive: Primitive,
        mark: &ScrollMark,
        offset: Point<ScaledPixels>,
        clip: Bounds<ScaledPixels>,
        images: &image_resources::ImageResources,
    ) {
        let shift = mark.moved + offset;
        let pivot = match &mark.pivot {
            Some(pivot) => record::geometry::scrolled(&mut primitive, shift, Some(pivot)).map(Rc::new),
            None => {
                record::geometry::primitive(&mut primitive, shift, None);
                None
            }
        };
        // Drawn with own ∩ chain: the chain is the mask it was drawn with, moved and clipped.
        let chain = primitive.content_mask().bounds.intersect(&clip);
        let own = Bounds { origin: mark.own.origin + shift, size: mark.own.size }.intersect(&clip);
        primitive.content_mask_mut().bounds = own;
        let outer = (self.scroll.open.replace((mark.node, chain)), self.scroll.pivot.take());
        self.scroll.pivot = pivot;
        self.insert_image_primitive(primitive, images);
        (self.scroll.open, self.scroll.pivot) = outer;
    }

    /// The landing map of a segment copy of this draft, moved by `offset`
    /// and clipped by `clip`, when the draft holds marks to follow.
    pub(super) fn segment_mapping(&self, offset: Point<ScaledPixels>, clip: Bounds<ScaledPixels>) -> Option<Box<Carry>> {
        (!self.scroll.marks.is_empty()).then(|| Carry::mapping(self.paint_operations.len(), offset, Some(clip)))
    }

    /// Relay the draft's nodes through the segment copy `output` made (`map`).
    pub(super) fn relay_segment(&self, output: &mut Scene, map: Option<Box<Carry>>) {
        let Some(map) = map.filter(|map| map.landed.len() == self.paint_operations.len()) else { return };
        let relayed = self.scroll.relayed.iter().map(|(space, hops)| (*space, hops.iter().cloned().chain([map.clone()]).collect()));
        output.scroll.relayed = relayed.collect();
        output.scroll.relayed.push((self.scroll.space, vec![map]));
    }
}