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
    /// or a reservoir with the primitives it drew (copies that dropped marks,
    /// layers or depth sorting need a build).
    pub(crate) fn scrollable(&self, range: &Range<usize>, nodes: impl Iterator<Item = Range<usize>>) -> bool {
        Self::shiftable(&self.paint_operations[range.clone()])
            && nodes.into_iter().all(|node| {
                let mut plain = 0;
                let mut at = node.start;
                while at < node.end {
                    match &self.paint_operations[at] {
                        PaintOperation::Reservoir(reservoir) => at += reservoir.drawn as usize,
                        PaintOperation::Primitive(_) | PaintOperation::Dormant(_) => plain += 1,
                        _ => return false,
                    }
                    at += 1;
                }
                at == node.end && plain == self.scroll.within(&node).len()
            })
    }

    /// `replay` of `range` of the rendered `prev` with the scroll nodes moved:
    /// `node` gives a node's shift and real clip chain in this frame. The
    /// commands of a node between other commands form a reservoir (made once
    /// from its marked primitives, then shared by the frames that replay it):
    /// each frame draws only its primitives that (own + shift) ∩ chain shows,
    /// found through its tile grid, in their paint order, moved by the
    /// node's total shift. Runs of commands with one shift are copied as
    /// `replay` copies a range, so a run keeps its relative orders and lies
    /// above what it overlaps before it. The caller checked `scrollable` and
    /// that this scene is at its root (no open layer or depth context).
    pub(crate) fn replay_scrolled(
        &mut self,
        range: Range<usize>,
        prev: &Scene,
        node: impl Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
        edges: &mut [(usize, usize)],
    ) {
        // Where each of dges (old command indices, sorted) lands here.
        let mut edge = 0;
        let marks = prev.scroll.within(&range);
        let mut marks = marks.iter().peekable();
        let mut run: Vec<PaintOperation> = Vec::new();
        let mut run_shift: Point<ScaledPixels> = Point::default();
        let mut at = range.start;
        while at < range.end {
            while edge < edges.len() && edges[edge].0 <= at {
                edges[edge].1 = self.paint_operations.len() + run.len();
                edge += 1;
            }
            let source = &prev.paint_operations[at];
            // A reservoir, or the marked primitives of one node made into one.
            let found = match source {
                PaintOperation::Reservoir(reservoir) => {
                    at += 1 + reservoir.drawn as usize;
                    Some(((**reservoir).clone(), node(reservoir.run.node)))
                }
                _ => match marks.peek() {
                    Some(mark) if mark.op as usize == at => {
                        let id = mark.node;
                        let mut items = Vec::new();
                        while let Some(mark) = marks.next_if(|mark| mark.op as usize == at && mark.node == id) {
                            items.push(Item::of(&prev.paint_operations[at], mark));
                            at += 1;
                        }
                        let run = std::rc::Rc::new(Run::new(id, items));
                        Some((ReservoirOp { run, shift: Point::default(), raised: 0, drawn: 0 }, node(id)))
                    }
                    _ => None,
                },
            };
            let Some((mut reservoir, (shift, chain))) = found else {
                if run_shift != Point::default() && !run.is_empty() {
                    self.replay_run(&run, &prev.book.images);
                    run.clear();
                }
                run_shift = Point::default();
                run.push(source.clone());
                at += 1;
                continue;
            };
            // A reservoir is copied on its own, indexed by its tree.
            if !run.is_empty() {
                self.replay_run(&run, &prev.book.images);
                run.clear();
            }
            run_shift = Point::default();
            reservoir.shift = reservoir.shift + shift;
            run.push(PaintOperation::Dormant(std::rc::Rc::new(Primitive::Quad(Default::default()))));
            reservoir.run.draw(reservoir.shift, reservoir.raised, chain, &mut run);
            reservoir.drawn = (run.len() - 1) as u32;
            let tree = reservoir.run.tree.clone();
            let graft = crate::bounds_tree::Graft::new(tree, reservoir.shift, Some(chain), reservoir.raised / ORDER_STEP);
            run[0] = PaintOperation::Reservoir(Box::new(reservoir));
            if !self.replay_shifted(&run, ReplaySource::Graft(graft, true), &prev.book.images, |_| true) {
                for operation in &run {
                    self.replay_image_operation(operation.clone(), &prev.book.images);
                }
            }
            run.clear();
        }
        if !run.is_empty() {
            self.replay_run(&run, &prev.book.images);
        }
        for edge in &mut edges[edge..] {
            edge.1 = self.paint_operations.len();
        }
    }

    /// One run of `replay_scrolled`, copied with a tree packed from it.
    fn replay_run(&mut self, run: &[PaintOperation], images: &image_resources::ImageResources) {
        if !self.replay_shifted(run, ReplaySource::Pack, images, |_| true) {
            for operation in run {
                self.replay_image_operation(operation.clone(), images);
            }
        }
    }
}

/// Side of a reservoir tile (scaled pixels). Like cc's raster tilings, and
/// unlike a one-axis sort, a grid finds what a clip shows when the node
/// scrolls on either axis (wide pages, carousels, vertical writing modes).
const TILE: f32 = 512.;
/// A primitive covering more tiles is checked on every draw instead.
const SPREAD: usize = 16;

/// One primitive of a reservoir, as of the frame it was made in.
#[derive(Clone)]
struct Item {
    primitive: Primitive,
    own: Bounds<ScaledPixels>,
    placed: Bounds<ScaledPixels>,
}

impl Item {
    /// A marked command with the shift and raise it has not taken yet.
    fn of(operation: &PaintOperation, mark: &ScrollMark) -> Self {
        let mut primitive = match operation {
            PaintOperation::Primitive(primitive) => primitive.clone(),
            PaintOperation::Dormant(primitive) => Primitive::clone(primitive),
            _ => unreachable!("a scroll mark on a command that is not a primitive"),
        };
        if mark.moved != Point::default() {
            record::geometry::primitive(&mut primitive, mark.moved, None);
        }
        *primitive.order_mut() += mark.raised;
        Item { primitive, own: mark.own, placed: mark.placed }
    }
}

/// The primitives one node painted between other commands, in paint order,
/// with a tile grid over where they land (CSR: `cells` offsets into `tiled`).
pub(crate) struct Run {
    node: ScrollNodeId,
    items: Vec<Item>,
    origin: Point<ScaledPixels>,
    columns: usize,
    rows: usize,
    cells: Vec<u32>,
    tiled: Vec<u32>,
    spread: Vec<u32>,
    /// Where the items land, as the bounds tree would hold them, packed once
    /// and grafted moved by the shift and clipped by the chain each frame.
    tree: std::rc::Rc<crate::bounds_tree::BoundsTree<ScaledPixels>>,
}

/// A reservoir in a scene: the shared run, the shift and order raise its
/// primitives take now, and how many primitives it drew after itself.
#[derive(Clone)]
pub(crate) struct ReservoirOp {
    pub(crate) run: std::rc::Rc<Run>,
    pub(crate) shift: Point<ScaledPixels>,
    pub(crate) raised: DrawOrder,
    pub(crate) drawn: u32,
}

impl Run {
    fn new(node: ScrollNodeId, items: Vec<Item>) -> Self {
        let area = items.iter().map(|item| item.placed.intersect(&item.own)).reduce(|a, b| a.union(&b));
        let area = area.unwrap_or_default();
        let tile = |value: ScaledPixels, from: ScaledPixels| ((value.0 - from.0) / TILE).floor().max(0.) as usize;
        let columns = tile(area.right(), area.left()) + 1;
        let rows = tile(area.bottom(), area.top()) + 1;
        let span = |bounds: &Bounds<ScaledPixels>| {
            let (left, right) = (tile(bounds.left(), area.left()), tile(bounds.right(), area.left()).min(columns - 1));
            let (top, bottom) = (tile(bounds.top(), area.top()), tile(bounds.bottom(), area.top()).min(rows - 1));
            (left, right, top, bottom)
        };
        let mut counts = vec![0u32; columns * rows + 1];
        let mut spread = Vec::new();
        let reach: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let shown = item.placed.intersect(&item.own);
                let (left, right, top, bottom) = span(&shown);
                let wide = shown.is_empty() || (right - left + 1) * (bottom - top + 1) > SPREAD;
                if wide {
                    spread.push(index as u32);
                } else {
                    for row in top..=bottom {
                        for column in left..=right {
                            counts[row * columns + column + 1] += 1;
                        }
                    }
                }
                (wide, left, right, top, bottom)
            })
            .collect();
        for cell in 1..counts.len() {
            counts[cell] += counts[cell - 1];
        }
        let mut fill = counts.clone();
        let mut tiled = vec![0u32; *counts.last().unwrap_or(&0) as usize];
        for (index, &(wide, left, right, top, bottom)) in reach.iter().enumerate() {
            if wide {
                continue;
            }
            for row in top..=bottom {
                for column in left..=right {
                    let cell = row * columns + column;
                    tiled[fill[cell] as usize] = index as u32;
                    fill[cell] += 1;
                }
            }
        }
        let rects = items
            .iter()
            .map(|item| (item.placed.intersect(&item.own), item.primitive.order() / ORDER_STEP))
            .filter(|(rect, _)| !rect.is_empty())
            .collect();
        let mut tree = crate::bounds_tree::BoundsTree::packed(rects, |coordinate| coordinate.0);
        tree.freeze();
        let tree = std::rc::Rc::new(tree);
        Run { node, items, origin: area.origin, columns, rows, cells: counts, tiled, spread, tree }
    }

    /// Append the primitives `chain` shows with the run moved by `shift`, in
    /// paint order, each moved and drawn with (own + shift) ∩ chain.
    fn draw(&self, shift: Point<ScaledPixels>, raised: DrawOrder, chain: Bounds<ScaledPixels>, out: &mut Vec<PaintOperation>) {
        // The clip in the run's coordinates, as tiles.
        let seen = record::geometry::bounds(chain, point(ScaledPixels(-shift.x.0), ScaledPixels(-shift.y.0)));
        let tile = |value: ScaledPixels, from: ScaledPixels| ((value.0 - from.0) / TILE).floor();
        let (left, right) = (tile(seen.left(), self.origin.x), tile(seen.right(), self.origin.x));
        let (top, bottom) = (tile(seen.top(), self.origin.y), tile(seen.bottom(), self.origin.y));
        let mut picked: Vec<u32> = self.spread.clone();
        if right >= 0. && bottom >= 0. && (left as usize) < self.columns && (top as usize) < self.rows && !seen.is_empty() {
            let clamp = |value: f32, count: usize| (value.max(0.) as usize).min(count - 1);
            for row in clamp(top, self.rows)..=clamp(bottom, self.rows) {
                let cells = row * self.columns;
                let (from, to) = (cells + clamp(left, self.columns), cells + clamp(right, self.columns));
                picked.extend_from_slice(&self.tiled[self.cells[from] as usize..self.cells[to + 1] as usize]);
            }
        }
        // Indices are paint order: sorting restores it, the grid only selects.
        picked.sort_unstable();
        picked.dedup();
        for index in picked {
            let item = &self.items[index as usize];
            let mask = item.own.intersect(&seen);
            if item.placed.intersect(&mask).is_empty() {
                continue;
            }
            let mut primitive = item.primitive.clone();
            record::geometry::primitive(&mut primitive, shift, None);
            *primitive.order_mut() += raised;
            primitive.content_mask_mut().bounds = record::geometry::bounds(mask, shift);
            out.push(PaintOperation::Primitive(primitive));
        }
    }
}