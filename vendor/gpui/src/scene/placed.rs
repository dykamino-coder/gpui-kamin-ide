//! KaminIDE patch: a range of the commands of a finished scene (a rendered
//! frame or a paint record) placed into another scene in one pass: moved by
//! an offset and indexed by one tree, with no intermediate scene. Fragment
//! pieces (a recorded child cut over columns) and blocks repeated at another
//! place of the next frame both use it.

use super::*;
use crate::bounds_tree::Graft;

/// What the content masks of a placed range become.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PlacedMask {
    /// The masks move with the commands and are cut by the clip, if any: a
    /// record placed under the clip of its destination, as `copy_record`
    /// places it (by any offset).
    Moves(Option<Bounds<ScaledPixels>>),
    /// Every command of the range has exactly this mask (the frame's) and
    /// keeps it: content that moves under a mask that stays. The offset must
    /// be a whole number of device pixels (glyphs and snapped edges painted
    /// afresh at the new place would not be the old ones moved by a fraction).
    Fixed(Bounds<ScaledPixels>),
}

/// Whether `primitive` puts the buffer of a paint group of its scene on screen.
fn group_output(primitive: &Primitive) -> bool {
    #[cfg(not(target_os = "macos"))]
    if let Primitive::Surface(surface) = primitive {
        return surface.group > 0;
    }
    let _ = primitive;
    false
}

/// Whether every mask of `primitive` is `mask`.
fn masked_by(primitive: &Primitive, mask: &Bounds<ScaledPixels>) -> bool {
    primitive.content_mask().bounds == *mask
        && match primitive {
            Primitive::Path(path) => path.vertices.iter().all(|v| v.content_mask.bounds == *mask),
            _ => true,
        }
}

/// Move `primitive` under a mask that stays (`PlacedMask::Fixed`).
fn move_under(primitive: &mut Primitive, offset: Point<ScaledPixels>, mask: Bounds<ScaledPixels>) {
    record::geometry::primitive(primitive, offset, None);
    let mask = ContentMask { bounds: mask };
    match primitive {
        Primitive::Shadow(p) => p.content_mask = mask,
        Primitive::Quad(p) => p.content_mask = mask,
        Primitive::Underline(p) => p.content_mask = mask,
        Primitive::MonochromeSprite(p) => p.content_mask = mask,
        Primitive::SubpixelSprite(p) => p.content_mask = mask,
        Primitive::PolychromeSprite(p) => p.content_mask = mask,
        Primitive::Surface(p) => p.content_mask = mask,
        Primitive::Path(p) => {
            for vertex in &mut p.vertices {
                vertex.content_mask = mask.clone();
            }
            p.content_mask = mask;
        }
    }
}

/// The rectangle the bounds tree holds for `primitive` moved under `mask`.
fn moved_under(primitive: &Primitive, offset: Point<ScaledPixels>, mask: &Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
    let placed = match primitive {
        // Not cloned for its vertices: a path is placed by its bounds.
        Primitive::Path(path) => record::geometry::bounds(path.bounds, offset),
        primitive => {
            let mut moved = primitive.clone();
            record::geometry::primitive(&mut moved, offset, None);
            placed_bounds(&moved)
        }
    };
    placed.intersect(mask)
}

impl Scene {
    /// The block of exactly the commands `ops` (blocks are in command order).
    fn block(&self, ops: &Range<usize>) -> Option<&ReplayedBlock> {
        let first = self.blocks.partition_point(|block| block.ops.start < ops.start);
        self.blocks[first..]
            .iter()
            .take_while(|block| block.ops.start == ops.start)
            .find(|block| block.ops == *ops)
    }

    /// Index the commands `ops` of this finished scene by a tree of their
    /// own, kept with the scene: every later `replay_placed_range` of the
    /// range grafts it instead of packing the commands again. A scene whose
    /// ranges are placed many times (a fragmented record, one range per
    /// segment) indexes each once. Nothing is done for a range that already
    /// has a block or cannot be placed as one.
    pub(crate) fn index_range(&mut self, ops: Range<usize>) {
        let Some(operations) = self.paint_operations.get(ops.clone()) else {
            return;
        };
        if self.block(&ops).is_some() || !Self::shiftable(operations) {
            return;
        }
        let mut packed = Self::packed(operations);
        packed.freeze();
        let index = match Graft::new(std::rc::Rc::new(packed), Point::default(), None, 0) {
            Some(graft) => BlockIndex::Owned(graft),
            None => BlockIndex::Empty,
        };
        let at = self.blocks.partition_point(|block| block.ops.start <= ops.start);
        self.blocks.insert(at, ReplayedBlock { ops, index });
    }

    /// Whether the commands `ops` of this finished scene can be placed
    /// elsewhere as one block: their orders come from the bounds tree and
    /// their layers are closed (`shiftable`: no bottom layer, no 3D depth
    /// sort), none puts a paint group's buffer on screen and the scene hoists
    /// no backdrop (both belong to this scene), and for `PlacedMask::Fixed`
    /// each has exactly that mask.
    pub(crate) fn placeable(&self, ops: Range<usize>, mask: &PlacedMask) -> bool {
        let Some(operations) = self.paint_operations.get(ops) else {
            return false;
        };
        self.hoisted_backdrops.is_empty()
            && Self::shiftable(operations)
            && operations.iter().all(|operation| match operation {
                PaintOperation::Primitive(primitive) => {
                    !group_output(primitive)
                        && match mask {
                            PlacedMask::Fixed(mask) => masked_by(primitive, mask),
                            PlacedMask::Moves(_) => true,
                        }
                }
                _ => true,
            })
    }

    /// The index of the commands `ops` of `source` moved by `offset` with
    /// their masks and cut by `clip`: a graft of the tree that indexed them
    /// (the whole scene's, or the range's block), else of a tree packed from
    /// them.
    fn moved_index(
        source: &Scene,
        ops: &Range<usize>,
        offset: Point<ScaledPixels>,
        clip: Option<Bounds<ScaledPixels>>,
    ) -> ReplaySource {
        // A finished frame has released its tree (`finish_frame`).
        if *ops == (0..source.paint_operations.len()) && !source.primitive_bounds.is_empty() {
            let graft = Graft::new(source.primitive_bounds.clone(), offset, clip, 0);
            return ReplaySource::Graft(graft, false);
        }
        match source.block(ops).map(|block| &block.index) {
            Some(BlockIndex::Empty) => return ReplaySource::Graft(None, false),
            Some(BlockIndex::Owned(graft)) => {
                return ReplaySource::Graft(graft.placed(&offset, clip), true);
            }
            Some(BlockIndex::Shared(graft)) => {
                if let Some(graft) = graft.upgrade() {
                    return ReplaySource::Graft(graft.placed(&offset, clip), false);
                }
            }
            None => {}
        }
        let mut packed = Self::packed(&source.paint_operations[ops.clone()]);
        packed.freeze();
        ReplaySource::Graft(Graft::new(std::rc::Rc::new(packed), offset, clip, 0), true)
    }

    /// The index of `operations` moved by `offset` under `mask`, which stays:
    /// a part the mask hid at the old place can be seen at the new one, so no
    /// tree of the old rectangles is exact; the moved ones are packed.
    fn fixed_index(
        operations: &[PaintOperation],
        offset: Point<ScaledPixels>,
        mask: &Bounds<ScaledPixels>,
    ) -> ReplaySource {
        let mut rects = Vec::new();
        let mut layers = 0usize;
        for operation in operations {
            match operation {
                PaintOperation::StartLayer(bounds, order) => {
                    if layers == 0 {
                        rects.push((record::geometry::bounds(*bounds, offset), order / ORDER_STEP));
                    }
                    layers += 1;
                }
                PaintOperation::EndLayer => layers -= 1,
                PaintOperation::Primitive(primitive) if layers == 0 => {
                    let clipped = moved_under(primitive, offset, mask);
                    if !clipped.is_empty() {
                        rects.push((clipped, primitive.order() / ORDER_STEP));
                    }
                }
                _ => {}
            }
        }
        let mut packed = BoundsTree::packed(rects, |coordinate| coordinate.0);
        packed.freeze();
        ReplaySource::Graft(Graft::new(std::rc::Rc::new(packed), Point::default(), None, 0), true)
    }

    /// `replay_placed(&isolated, offset, clip)` of the isolated scene a plan
    /// paints `record` into under the mask `extent` (`Window::paint_plan`),
    /// without that scene: each command takes the same two placements in the
    /// same order (cut by `extent` in place, then moved by `offset` and cut
    /// by `clip`), and the record's tree is grafted through both, as the
    /// graft of a scene that is one graft collapses. `false`, and nothing is
    /// written, where either of the two replays would decline.
    pub(crate) fn replay_record_placed(
        &mut self,
        record: &Scene,
        extent: Bounds<ScaledPixels>,
        offset: Point<ScaledPixels>,
        clip: Bounds<ScaledPixels>,
    ) -> bool {
        if !record.groups.is_empty() || !record.hoisted_backdrops.is_empty() {
            return false;
        }
        let graft = Graft::new(record.primitive_bounds.clone(), Point::default(), Some(extent), 0)
            .and_then(|graft| graft.placed(&offset, Some(clip)));
        let source = ReplaySource::Graft(graft, false);
        self.replay_shifted(&record.paint_operations, source, |operation| match operation {
            PaintOperation::Primitive(primitive) => {
                record::geometry::primitive(primitive, Point::default(), Some(extent));
                record::geometry::primitive(primitive, offset, Some(clip));
                !placed_bounds(primitive).intersect(&primitive.content_mask().bounds).is_empty()
            }
            PaintOperation::StartLayer(bounds, _) => {
                *bounds = record::geometry::bounds(*bounds, offset);
                true
            }
            _ => true,
        })
    }

    /// Append the commands `ops` of the finished scene `source`, moved by
    /// `offset`, in one pass, with one index for all of them: the commands
    /// keep their recorded orders under one raise (`replay_shifted`), and
    /// later inserts see exactly the rectangles the moved commands cover.
    /// Commands their mask hides are dropped, as `insert_primitive` drops
    /// them. `false`, and nothing is written, when the range is not
    /// `placeable`, the offset of a `PlacedMask::Fixed` range is not a whole
    /// number of device pixels, or this scene's open state (a layer, a 3D
    /// context) needs the per-primitive replay.
    pub(crate) fn replay_placed_range(
        &mut self,
        source: &Scene,
        ops: Range<usize>,
        offset: Point<ScaledPixels>,
        mask: PlacedMask,
    ) -> bool {
        if !source.placeable(ops.clone(), &mask) {
            return false;
        }
        let operations = &source.paint_operations[ops.clone()];
        let index = match &mask {
            PlacedMask::Moves(clip) => Self::moved_index(source, &ops, offset, *clip),
            PlacedMask::Fixed(_) if offset.x.0.fract() != 0.0 || offset.y.0.fract() != 0.0 => return false,
            PlacedMask::Fixed(mask) => Self::fixed_index(operations, offset, mask),
        };
        self.replay_shifted(operations, index, |operation| match operation {
            PaintOperation::Primitive(primitive) => {
                match mask {
                    PlacedMask::Moves(clip) => record::geometry::primitive(primitive, offset, clip),
                    PlacedMask::Fixed(mask) => move_under(primitive, offset, mask),
                }
                !placed_bounds(primitive).intersect(&primitive.content_mask().bounds).is_empty()
            }
            PaintOperation::StartLayer(bounds, _) => {
                *bounds = record::geometry::bounds(*bounds, offset);
                true
            }
            _ => true,
        })
    }
}

#[cfg(test)]
#[path = "placed_tests.rs"]
mod tests;
