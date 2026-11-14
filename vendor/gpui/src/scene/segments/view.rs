//! KaminIDE patch: a segment kept as a selection of its draft's commands
//! instead of a scene of copies (`freeze_segment`). The draft is recorded
//! once for all the segments of a fragmented box; a segment's commands are
//! spread over it in paint order (backgrounds of every column, then floats,
//! then text), so a segment is a list of picks, not a range. A pick is placed
//! when the segment is painted: one write of each command into the frame.

use super::*;
use crate::bounds_tree::Graft;

/// One command of the draft admitted to the segment, and its clip there.
struct Pick {
    op: u32,
    clip: Bounds<ScaledPixels>,
}

pub(crate) struct SegmentView {
    picks: Vec<Pick>,
    /// The move of every pick from the draft to the segment.
    delta: Point<ScaledPixels>,
    /// The rectangles of the picks in the segment, with their draft orders:
    /// the picks move together, so the orders keep the overlap rule there.
    tree: Rc<BoundsTree<ScaledPixels>>,
}

impl Scene {
    /// The segment `id` of this draft as a selection, clipped by `extent`
    /// like `freeze_segment` clips it. `None` when the segment needs the
    /// frozen scene: the draft has paint groups, hoisted backdrops or layers,
    /// or the segment places a command twice or by different moves (the
    /// draft orders would not order them).
    pub(crate) fn segment_view(
        &self,
        id: PaintSegmentId,
        scale: f32,
        extent: Bounds<ScaledPixels>,
    ) -> Option<SegmentView> {
        let capture = self.segment_capture.as_ref()?;
        if !self.groups.is_empty()
            || !self.hoisted_backdrops.is_empty()
            || capture.primitives.len() != self.paint_operations.len()
        {
            return None;
        }
        let mut picks = Vec::new();
        let mut rects = Vec::new();
        let mut delta = None;
        for (op, (operation, tag)) in self.paint_operations.iter().zip(&capture.primitives).enumerate() {
            let PaintOperation::Primitive(primitive) = operation else {
                return None;
            };
            let mut routes = tag.routes.iter().filter(|route| route.id == id).copied();
            let Some(route) = routes.next() else {
                continue;
            };
            let moved = (route.destination - route.source).scale(scale);
            if routes.next().is_some() || *delta.get_or_insert(moved) != moved {
                return None;
            }
            let clip = Tag::clip_chain(tag.clips.as_ref(), None, route, scale, extent);
            let clip = tag.admission(route, scale, clip);
            let mut placed = primitive.clone();
            record::geometry::primitive(&mut placed, moved, Some(clip));
            let rect = placed_bounds(&placed).intersect(&placed.content_mask().bounds);
            if !rect.is_empty() {
                picks.push(Pick { op: op as u32, clip });
                rects.push((rect, primitive.order() / ORDER_STEP));
            }
        }
        picks.shrink_to_fit();
        let mut tree = BoundsTree::packed(rects, |coordinate| coordinate.0);
        tree.freeze();
        Some(SegmentView { picks, delta: delta.unwrap_or_default(), tree: Rc::new(tree) })
    }

    /// Keep of a draft only what its segment views and a later
    /// `freeze_segment` read: the command log, without the per-kind lists and
    /// the index of a scene that is never drawn itself.
    pub(crate) fn keep_commands(&mut self) {
        self.primitive_bounds = Default::default();
        self.blocks = Vec::new();
        self.shadows = Vec::new();
        self.quads = Vec::new();
        self.paths = Vec::new();
        self.underlines = Vec::new();
        self.monochrome_sprites = Vec::new();
        self.subpixel_sprites = Vec::new();
        self.polychrome_sprites = Vec::new();
        self.surfaces = Vec::new();
    }

    /// `replay_record_placed` of `draft.freeze_segment(..)` without the
    /// frozen scene: every pick takes the placements its copy would take, in
    /// the same order (moved into the segment under its clip, cut by
    /// `extent`, moved by `offset` and cut by `clip`). The picks keep their
    /// draft orders under one raise. `false`, and nothing is written, when
    /// this scene's open state needs the per-primitive replay.
    pub(crate) fn replay_view(
        &mut self,
        draft: &Scene,
        view: &SegmentView,
        extent: Bounds<ScaledPixels>,
        offset: Point<ScaledPixels>,
        clip: Bounds<ScaledPixels>,
    ) -> bool {
        if !self.takes_blocks() {
            return false;
        }
        let graft = Graft::new(view.tree.clone(), Point::default(), Some(extent), 0)
            .and_then(|graft| graft.placed(&offset, Some(clip)));
        let placed = view.picks.iter().filter_map(|pick| {
            let PaintOperation::Primitive(primitive) = &draft.paint_operations[pick.op as usize] else {
                return None;
            };
            let mut primitive = primitive.clone();
            record::geometry::primitive(&mut primitive, view.delta, Some(pick.clip));
            record::geometry::primitive(&mut primitive, Point::default(), Some(extent));
            record::geometry::primitive(&mut primitive, offset, Some(clip));
            let seen = !placed_bounds(&primitive).intersect(&primitive.content_mask().bounds).is_empty();
            seen.then_some(PaintOperation::Primitive(primitive))
        });
        self.append_block(placed, graft, false);
        true
    }
}
