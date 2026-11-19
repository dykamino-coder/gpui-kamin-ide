//! KaminIDE patch: primitives painted under a scroll node (`window::scroll_nodes`).
//!
//! A primitive painted inside a scroll container keeps its own mask: the mask
//! its element built under the node's interest rect, without the node's real
//! clip chain. The scene draws it with own ∩ chain, and keeps a primitive whose
//! drawn mask is empty but whose own mask is not as a dormant command (in the
//! command list, not in the draw lists), so a replay at another scroll offset
//! can draw what scrolls into view without painting the subtree again.
//!
//! A dormant primitive is shared (`Rc`) between the frames that keep it and
//! is not touched while it stays dormant: its mark carries the shift and the
//! order raise still to apply, so a step costs a rectangle test per dormant
//! command, not a copy of it.

use super::*;
use crate::ScrollNodeId;

/// The scroll node and own mask of one command (a primitive or a dormant one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrollMark {
    /// Index of the command in `paint_operations`.
    pub(crate) op: u32,
    pub(crate) node: ScrollNodeId,
    pub(crate) own: Bounds<ScaledPixels>,
    /// Where the primitive lands now (`placed_bounds` after `moved`).
    pub(crate) placed: Bounds<ScaledPixels>,
    /// Of a dormant primitive: the shift its geometry has not taken yet.
    pub(crate) moved: Point<ScaledPixels>,
    /// Of a dormant primitive: the order raise it has not taken yet.
    pub(crate) raised: DrawOrder,
}

/// The scroll marks of a scene, sorted by command.
#[derive(Default)]
pub(crate) struct ScrollBook {
    pub(crate) marks: Vec<ScrollMark>,
    /// The node a primitive inserted now paints under, with that node's real
    /// clip chain (set by `Window::insert_scene_primitive` around one insert).
    pub(crate) open: Option<(ScrollNodeId, Bounds<ScaledPixels>)>,
    /// The order raise of the last `replay_shifted` copy (0 for any other).
    pub(crate) raise: DrawOrder,
}

impl ScrollBook {
    pub(crate) fn clear(&mut self) {
        self.marks.clear();
        self.open = None;
    }

    /// Marks of the commands in `range`.
    pub(crate) fn within(&self, range: &Range<usize>) -> &[ScrollMark] {
        let from = self.marks.partition_point(|mark| (mark.op as usize) < range.start);
        let to = self.marks.partition_point(|mark| (mark.op as usize) < range.end);
        &self.marks[from..to]
    }
}

impl Scene {
    /// `insert_primitive` under a scroll node: draw with own ∩ the node's
    /// chain; order by own (a later replay may show more of it, and overlaps
    /// only grow, so orders stay valid); keep an invisible primitive with a
    /// visible own mask dormant. `false` when it was neither drawn nor kept.
    pub(super) fn mark_scrolled(
        &mut self,
        primitive: &mut Primitive,
        node: ScrollNodeId,
        chain: Bounds<ScaledPixels>,
    ) -> Option<Bounds<ScaledPixels>> {
        let own = primitive.content_mask().bounds;
        primitive.content_mask_mut().bounds = own.intersect(&chain);
        let placed = placed_bounds(primitive);
        let ordered = placed.intersect(&own);
        let at = self.paint_operations.len() as u32;
        if placed.intersect(&primitive.content_mask().bounds).is_empty() {
            // Dormant only where its order can come from the bounds tree.
            if !ordered.is_empty() && self.layer_stack.is_empty() && self.depth.contexts.is_empty() {
                *primitive.order_mut() = self.tree_mut().insert(ordered) * ORDER_STEP;
                self.paint_operations.push(PaintOperation::Dormant(std::rc::Rc::new(primitive.clone())));
                self.scroll.marks.push(ScrollMark { op: at, node, own, placed, moved: Point::default(), raised: 0 });
            }
            return None;
        }
        self.scroll.marks.push(ScrollMark { op: at, node, own, placed, moved: Point::default(), raised: 0 });
        Some(ordered)
    }

    /// After a command range of `source` was copied here starting at
    /// `landed`: copy its marks when the copy was one command per command.
    pub(crate) fn copy_scroll_marks(&mut self, source: &Scene, range: Range<usize>, landed: usize) {
        let marks = source.scroll.within(&range);
        if marks.is_empty() || self.paint_operations.len() - landed != range.len() {
            return;
        }
        let shift = |op: u32| (landed + op as usize - range.start) as u32;
        let raise = self.scroll.raise;
        let ops = &self.paint_operations;
        self.scroll.marks.extend(marks.iter().map(|mark| {
            let op = shift(mark.op);
            let dormant = matches!(ops[op as usize], PaintOperation::Dormant(_));
            ScrollMark { op, raised: mark.raised + if dormant { raise } else { 0 }, ..*mark }
        }));
    }

    /// Whether no layer or depth context is open (a scrolled replay copies
    /// runs as `replay` does at the root).
    pub(crate) fn at_root(&self) -> bool {
        self.layer_stack.is_empty() && self.depth.contexts.is_empty() && self.depth.plane_stack.is_empty()
    }

    /// Whether `range` of this finished scene can be replayed scrolled: its
    /// commands keep their orders under one shift (`shiftable`), and inside
    /// each node's scene range (`nodes`) every command is a marked primitive
    /// (copies that dropped marks, layers or depth sorting need a build).
    pub(crate) fn scrollable(&self, range: &Range<usize>, nodes: impl Iterator<Item = Range<usize>>) -> bool {
        let operations = &self.paint_operations[range.clone()];
        Self::shiftable(operations)
            && nodes.into_iter().all(|node| {
                let marked = self.scroll.within(&node).len();
                marked == node.len()
                    && self.paint_operations[node]
                        .iter()
                        .all(|op| matches!(op, PaintOperation::Primitive(_) | PaintOperation::Dormant(_)))
            })
    }

    /// `replay` of `range` of the rendered `prev` with the scroll nodes moved:
    /// `node` gives a node's shift and real clip chain in this frame. Each
    /// marked primitive moves by its node's shift and is drawn with
    /// (own + shift) ∩ chain, dormant when that shows nothing; runs of
    /// commands with one shift are copied as `replay` copies a range, so a run
    /// keeps its recorded relative orders and lies above what it overlaps
    /// before it. The caller checked `scrollable` and that this scene is at
    /// its root (no open layer or depth context).
    pub(crate) fn replay_scrolled(
        &mut self,
        range: Range<usize>,
        prev: &Scene,
        node: impl Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
    ) {
        let marks = prev.scroll.within(&range);
        let mut marks = marks.iter().peekable();
        let mut run: Vec<PaintOperation> = Vec::new();
        let mut run_marks: Vec<ScrollMark> = Vec::new();
        let mut run_shift = Point::default();
        for at in range {
            let source = &prev.paint_operations[at];
            let Some(found) = marks.next_if(|mark| mark.op as usize == at) else {
                if run_shift != Point::default() && !run.is_empty() {
                    self.replay_run(&run, &run_marks, &prev.book.images);
                    run.clear();
                    run_marks.clear();
                }
                run_shift = Point::default();
                run.push(source.clone());
                continue;
            };
            let (shift, chain) = node(found.node);
            let own = record::geometry::bounds(found.own, shift);
            let placed = record::geometry::bounds(found.placed, shift);
            let mask = own.intersect(&chain);
            let shown = !placed.intersect(&mask).is_empty();
            let mut mark = ScrollMark { own, placed, ..*found };
            let operation = match (source, shown) {
                (PaintOperation::Primitive(primitive), true) => {
                    let mut primitive = primitive.clone();
                    record::geometry::primitive(&mut primitive, shift, None);
                    primitive.content_mask_mut().bounds = mask;
                    PaintOperation::Primitive(primitive)
                }
                (PaintOperation::Primitive(primitive), false) => {
                    mark.moved = shift;
                    PaintOperation::Dormant(std::rc::Rc::new(primitive.clone()))
                }
                (PaintOperation::Dormant(primitive), true) => {
                    let mut primitive = Primitive::clone(primitive);
                    record::geometry::primitive(&mut primitive, found.moved + shift, None);
                    *primitive.order_mut() += found.raised;
                    primitive.content_mask_mut().bounds = mask;
                    mark.moved = Point::default();
                    mark.raised = 0;
                    PaintOperation::Primitive(primitive)
                }
                (PaintOperation::Dormant(primitive), false) => {
                    mark.moved = found.moved + shift;
                    PaintOperation::Dormant(primitive.clone())
                }
                _ => unreachable!("a scroll mark on a command that is not a primitive"),
            };
            if shift != run_shift && !run.is_empty() {
                self.replay_run(&run, &run_marks, &prev.book.images);
                run.clear();
                run_marks.clear();
            }
            run_shift = shift;
            mark.op = run.len() as u32;
            run_marks.push(mark);
            run.push(operation);
        }
        if !run.is_empty() {
            self.replay_run(&run, &run_marks, &prev.book.images);
        }
    }
    /// One run of `replay_scrolled`: copied with a tree packed from the
    /// commands; marks land when the copy was one command per command.
    fn replay_run(&mut self, run: &[PaintOperation], marks: &[ScrollMark], images: &image_resources::ImageResources) {
        let landed = self.paint_operations.len();
        self.scroll.raise = 0;
        if !self.replay_shifted(run, ReplaySource::Pack, images, |_| true) {
            for operation in run {
                self.replay_image_operation(operation.clone(), images);
            }
        }
        if self.paint_operations.len() - landed == run.len() {
            let raise = self.scroll.raise;
            let ops = &self.paint_operations;
            self.scroll.marks.extend(marks.iter().map(|mark| {
                let op = landed as u32 + mark.op;
                let dormant = matches!(ops[op as usize], PaintOperation::Dormant(_));
                ScrollMark { op, raised: mark.raised + if dormant { raise } else { 0 }, ..*mark }
            }));
        }
    }
}