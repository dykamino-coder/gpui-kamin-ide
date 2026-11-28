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

pub(crate) mod carry;
mod check;
mod depth;
#[cfg(test)]
mod live_tests;
pub(crate) mod pivots;
use super::*;
use crate::ScrollNodeId;

/// The scroll node and own mask of one command (a primitive or a dormant one).
#[derive(Clone, Debug, PartialEq)]
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
    /// Of a primitive drawn through a matrix a shift cannot move exactly:
    /// the inputs of that matrix at the geometry the primitive holds, so a
    /// move builds it again as a fresh paint there does (`TransformPivot`).
    pub(crate) pivot: Option<std::rc::Rc<TransformPivot>>,
}

mod space;
pub(crate) use space::SceneSpace;

/// The scroll marks of a scene, sorted by command.
#[derive(Default)]
pub(crate) struct ScrollBook {
    pub(crate) marks: Vec<ScrollMark>,
    pub(crate) space: SceneSpace,
    /// The node a primitive inserted now paints under, with that node's real
    /// clip chain (set by `Window::insert_scene_primitive` around one insert).
    pub(crate) open: Option<(ScrollNodeId, Bounds<ScaledPixels>)>,
    /// The order raise of the last `replay_shifted` copy (0 for any other).
    pub(crate) raise: DrawOrder,
    /// A primitive under `open` that a later offset may show could not be
    /// kept (a bottom layer or depth context): its node cannot be replayed
    /// moved (`Window::taint_open_scroll_node`).
    pub(crate) lost: bool,
    /// Of a paint record's scene: how many paint nodes were open when it
    /// began recording. A record lands later, maybe at another offset, under
    /// whatever node is open there, so what it paints under those outer
    /// nodes keeps its own mask, unmarked and never dormant, and is marked
    /// where it lands (`Window::with_copies_marked`); a node opened inside
    /// the record marks its commands as usual.
    pub(crate) record_base: Option<usize>,
    /// A node paired inside this scene, including nodes with no commands.
    pub(crate) has_nodes: bool,
    /// While a paint record lands: the marks of the nodes it opened inside
    /// itself, carried to its copy (carry).
    pub(crate) carry: Option<Box<carry::Carry>>,
    /// Of a paint record's scene: the records landed in it whose nodes it
    /// holds, by their scene, and each landing on the way (carry::Relay).
    pub(crate) relayed: Vec<carry::Relay>,
    /// Around one insert (`Window::insert_scene_primitive`): the pivot of
    /// the flat transform painted now.
    pub(crate) pivot: Option<std::rc::Rc<TransformPivot>>,
    /// A primitive marked under the open node is drawn through a matrix a
    /// shift cannot move exactly, without its pivot: the node cannot be
    /// replayed moved (`Window::note_unpivoted`).
    pub(crate) unpivoted: bool,
    /// Of a paint record's scene: the pivots of the unmarked primitives it
    /// holds (painted under the nodes open before it began), by command, so
    /// a copy marked where it lands keeps them (`copying`).
    pub(crate) pivots: Vec<(u32, std::rc::Rc<TransformPivot>)>,
    /// While a finished scene is copied under the open node: the pivots of
    /// its commands and the offset the copy lands at (`mark_copied`).
    pub(crate) copying: Option<Box<pivots::Copying>>,
}

impl ScrollBook {
    pub(crate) fn clear(&mut self) {
        self.marks.clear();
        self.open = None;
        self.carry = None;
        self.relayed.clear();
        self.has_nodes = false;
        (self.pivot, self.unpivoted) = (None, false);
        self.pivots.clear();
        self.copying = None;
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
        let pivot = self.pivot_of(primitive);
        if placed.intersect(&primitive.content_mask().bounds).is_empty() {
            if ordered.is_empty() {
                return None;
            }
            // Dormant where its order is known: from the bounds tree, or the
            // order of the layer it paints in (a text line, `Window::paint_layer`).
            let order = match self.layer_stack.last().copied() {
                None => Some(self.tree_mut().insert(ordered) * ORDER_STEP),
                Some(BOTTOM_LAYER) => None,
                Some(layer) => Some(layer),
            };
            let Some(order) = order else {
                self.scroll.lost = true;
                return None;
            };
            *primitive.order_mut() = order;
            self.paint_operations.push(PaintOperation::Dormant(std::rc::Rc::new(primitive.clone())));
            self.scroll.marks.push(ScrollMark { op: at, node, own, placed, moved: Point::default(), raised: 0, pivot });
            return None;
        }
        self.scroll.marks.push(ScrollMark { op: at, node, own, placed, moved: Point::default(), raised: 0, pivot });
        Some(ordered)
    }

    /// The pivot a mark of `primitive` inserted now keeps: the open one when
    /// the primitive holds its matrix. A primitive a shift cannot move
    /// exactly without one notes its node (`ScrollBook::unpivoted`).
    fn pivot_of(&mut self, primitive: &Primitive) -> Option<std::rc::Rc<TransformPivot>> {
        // A primitive without a matrix (a shadow, path, underline or surface)
        // is placed by its bounds alone: a shift moves it exactly.
        let matrix = primitive.transformation();
        if matrix.is_none_or(TransformationMatrix::moves_exactly) {
            return None;
        }
        let pivot = self.scroll.pivot.as_ref().filter(|pivot| matrix == Some(&pivot.matrix())).cloned();
        self.scroll.unpivoted |= pivot.is_none();
        pivot
    }

    /// `replay_shifted` of a copy under the open node: mark a primitive (its
    /// mask now is its own: the order came from it) and draw it with own ∩
    /// chain, or turn it dormant (`true`, raised by `raise`: the caller keeps
    /// it as it is); mark a layer. `index`: the command copied, in its source.
    pub(super) fn mark_copied(&mut self, operation: &mut PaintOperation, raise: DrawOrder, index: usize) -> bool {
        let Some((node, chain)) = self.scroll.open else { return false };
        let op = self.paint_operations.len() as u32;
        match operation {
            PaintOperation::Primitive(primitive) => {
                // The pivot of source command `index` (`copying_pivots`) builds its matrix.
                let pivot = self.copied_pivot(primitive, index, true);
                let own = primitive.content_mask().bounds;
                primitive.content_mask_mut().bounds = own.intersect(&chain);
                let placed = placed_bounds(primitive);
                self.scroll.marks.push(ScrollMark { op, node, own, placed, moved: Point::default(), raised: 0, pivot });
                if !placed.intersect(&primitive.content_mask().bounds).is_empty() {
                    return false;
                }
                *primitive.order_mut() += raise;
                *operation = PaintOperation::Dormant(std::rc::Rc::new(primitive.clone()));
                true
            }
            PaintOperation::StartLayer(bounds, _) => {
                let mark = ScrollMark { op, node, own: *bounds, placed: *bounds, moved: Point::default(), raised: 0, pivot: None };
                self.scroll.marks.push(mark);
                false
            }
            _ => false,
        }
    }

    /// Mark the layer `push_layer` opens now under the open node: its bounds
    /// (clipped by the own mask) move with the node, as its primitives do.
    pub(super) fn mark_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        if let Some((node, _)) = self.scroll.open {
            let op = self.paint_operations.len() as u32;
            let mark = ScrollMark { op, node, own: bounds, placed: bounds, moved: Point::default(), raised: 0, pivot: None };
            self.scroll.marks.push(mark);
        }
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
            ScrollMark { op, raised: mark.raised + if dormant { raise } else { 0 }, ..mark.clone() }
        }));
    }

    /// The commands of `range` as a moved copy places them: a primitive
    /// painted under a scroll node, dormant or not, with its own mask (the
    /// copy is clipped and marked where it lands), a reservoir as all it
    /// holds instead of what it drew.
    pub(crate) fn unscrolled(&self, range: Range<usize>) -> Vec<PaintOperation> {
        let mut marks = self.scroll.within(&range).iter().peekable();
        let mut out = Vec::with_capacity(range.len());
        let mut at = range.start;
        while at < range.end {
            let mark = marks.next_if(|mark| mark.op as usize == at);
            match (&self.paint_operations[at], mark) {
                (PaintOperation::Reservoir(reservoir), _) => {
                    reservoir.run.expand(reservoir.shift, reservoir.raised, &mut out);
                    at += reservoir.drawn as usize;
                }
                (operation @ (PaintOperation::Primitive(_) | PaintOperation::Dormant(_)), Some(mark)) => {
                    let Piece::Primitive(mut primitive) = Item::of(operation, mark).piece else { unreachable!() };
                    primitive.content_mask_mut().bounds = mark.own;
                    out.push(PaintOperation::Primitive(primitive));
                }
                (operation, _) => out.push(operation.clone()),
            }
            at += 1;
        }
        out
    }

    /// The commands of this finished scene (a paint record) for a copy under
    /// the open node `node`: a primitive it marked under `node` with its own
    /// mask, dormant or not, so the copy is marked where it lands
    /// (`mark_copied`), as `unscrolled` gives a moved copy its commands. A
    /// primitive of another node keeps the mask its node clipped it with,
    /// and stays unmarked: a scrolled replay of the frame then builds.
    /// `None` when there is nothing to give back: no mark under `node`, or a
    /// reservoir, whose run the grafted tree does not index.
    pub(crate) fn unscrolled_under(&self, node: ScrollNodeId) -> Option<Vec<PaintOperation>> {
        if !self.scroll.marks.iter().any(|mark| mark.node == node)
            || self.paint_operations.iter().any(|op| matches!(op, PaintOperation::Reservoir(_)))
        {
            return None;
        }
        let mut marks = self.scroll.marks.iter().peekable();
        let out = self.paint_operations.iter().enumerate().map(|(at, operation)| {
            let mark = marks.next_if(|mark| mark.op as usize == at).filter(|mark| mark.node == node);
            match (operation, mark) {
                (PaintOperation::Primitive(_) | PaintOperation::Dormant(_), Some(mark)) => {
                    let Piece::Primitive(mut primitive) = Item::of(operation, mark).piece else { unreachable!() };
                    primitive.content_mask_mut().bounds = mark.own;
                    PaintOperation::Primitive(primitive)
                }
                _ => operation.clone(),
            }
        });
        Some(out.collect())
    }

    /// Whether no layer or depth context is open (a scrolled replay copies
    /// runs as `replay` does at the root).
    pub(crate) fn at_root(&self) -> bool {
        self.layer_stack.is_empty() && self.depth.contexts.is_empty() && self.depth.plane_stack.is_empty()
    }

    /// `replay` of `range` of the rendered `prev` with the scroll nodes moved:
    /// `node` gives a node's shift and real clip chain in this frame. The
    /// commands of a node between other commands form a reservoir (made once
    /// from its marked primitives, then shared by the frames that replay it):
    /// each frame draws only its primitives that (own + shift) ∩ chain shows,
    /// found through its tile grid, in their paint order, moved by the
    /// node's total shift. Flat runs keep their relative orders; depth
    /// metadata orders newly visible primitives in their original planes.
    /// The destination starts at its root (no open layer or depth context).
    pub(crate) fn replay_scrolled(
        &mut self,
        range: Range<usize>,
        prev: &Scene,
        node: impl Fn(ScrollNodeId) -> (Point<ScaledPixels>, Bounds<ScaledPixels>),
        edges: &mut [(usize, usize)],
    ) {
        if prev.has_depth(&range) {
            return self.replay_depth_scrolled(range, prev, &node, edges);
        }
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
                        let items = Item::collect(&prev.paint_operations, &mut marks, &mut at, id);
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
            let source = if reservoir.run.live {
                // The old index is valid only for translation. A spatially
                // rebuilt projection orders against its current drawn bounds.
                ReplaySource::Pack
            } else {
                let tree = reservoir.run.tree.clone();
                let graft = crate::bounds_tree::Graft::new(tree, reservoir.shift, Some(chain), reservoir.raised / ORDER_STEP);
                ReplaySource::Graft(graft, true)
            };
            run[0] = PaintOperation::Reservoir(Box::new(reservoir));
            if !self.replay_shifted(&run, source, &prev.book.images, |_| true) {
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

/// One command of a reservoir, as of the frame it was made in.
#[derive(Clone)]
pub(crate) enum Piece {
    Primitive(Primitive),
    /// A layer (a text line) and its order; its commands follow up to `end`.
    Layer(DrawOrder),
    End,
}

/// One command of a reservoir with its own mask and where it lands.
#[derive(Clone)]
pub(crate) struct Item {
    pub(crate) piece: Piece,
    own: Bounds<ScaledPixels>,
    placed: Bounds<ScaledPixels>,
    /// Of a layer: the index past its `End`.
    end: u32,
    /// The pivot of the primitive's matrix at the geometry it holds.
    pivot: Option<std::rc::Rc<TransformPivot>>,
}

impl Item {
    /// A marked primitive with the shift and raise it has not taken yet.
    fn of(operation: &PaintOperation, mark: &ScrollMark) -> Self {
        let mut primitive = match operation {
            PaintOperation::Primitive(primitive) => primitive.clone(),
            PaintOperation::Dormant(primitive) => Primitive::clone(primitive),
            _ => unreachable!("a scroll mark on a command that is not a primitive"),
        };
        let pivot = match mark.moved == Point::default() {
            true => mark.pivot.clone(),
            false => record::geometry::scrolled(&mut primitive, mark.moved, mark.pivot.as_deref()).map(std::rc::Rc::new),
        };
        *primitive.order_mut() += mark.raised;
        Item { piece: Piece::Primitive(primitive), own: mark.own, placed: mark.placed, end: 0, pivot }
    }

    /// The marked commands of node `id` from `at` on (`Scene::scrollable`
    /// checked their shape): primitives, and layers with what they hold.
    fn collect<'a>(
        operations: &[PaintOperation],
        marks: &mut std::iter::Peekable<impl Iterator<Item = &'a ScrollMark>>,
        at: &mut usize,
        id: ScrollNodeId,
    ) -> Vec<Item> {
        let mut items = Vec::new();
        // Indices of the layer items open now.
        let mut open: Vec<usize> = Vec::new();
        loop {
            let here = *at;
            let mark = if open.is_empty() {
                marks.next_if(|mark| mark.op as usize == here && mark.node == id)
            } else {
                marks.next_if(|mark| mark.op as usize == here)
            };
            match (&operations[here], mark) {
                (PaintOperation::StartLayer(bounds, order), Some(mark)) => {
                    open.push(items.len());
                    items.push(Item { piece: Piece::Layer(*order), own: mark.own, placed: *bounds, end: 0, pivot: None });
                }
                (PaintOperation::EndLayer, None) if !open.is_empty() => {
                    items.push(Item { piece: Piece::End, own: Bounds::default(), placed: Bounds::default(), end: 0, pivot: None });
                    let start = open.pop().unwrap_or_default();
                    items[start].end = items.len() as u32;
                }
                (operation, Some(mark)) => items.push(Item::of(operation, mark)),
                _ => break,
            }
            *at += 1;
            if *at == operations.len() {
                break;
            }
        }
        items
    }
}

/// The primitives one node painted between other commands, in paint order,
/// with a tile grid over where they land (CSR: `cells` offsets into `tiled`).
#[derive(Clone)] pub(crate) struct Run {
    pub(crate) node: ScrollNodeId,
    pub(crate) items: Vec<Item>,
    origin: Point<ScaledPixels>,
    columns: usize,
    rows: usize,
    cells: Vec<u32>,
    tiled: Vec<u32>,
    spread: Vec<u32>,
    /// A pivot can change its projection as its spatial inputs move. Its
    /// old bounds translated by the scroll delta cannot index that geometry.
    live: bool,
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
        let live = items.iter().any(|item| item.pivot.is_some());
        // The grid and the tree hold the top commands: a layer stands for
        // what it holds, as `Scene::packed` and the bounds tree see it.
        let mut top = Vec::with_capacity(items.len());
        let mut at = 0;
        while at < items.len() {
            top.push(at as u32);
            at = match items[at].piece {
                Piece::Layer(_) => (items[at].end as usize).max(at + 1),
                _ => at + 1,
            };
        }
        let area = top.iter().map(|&at| &items[at as usize]).map(|item| item.placed.intersect(&item.own)).reduce(|a, b| a.union(&b));
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
        let reach: Vec<_> = top
            .iter()
            .map(|&index| {
                let shown = items[index as usize].placed.intersect(&items[index as usize].own);
                let (left, right, top, bottom) = span(&shown);
                let wide = live || shown.is_empty() || (right - left + 1) * (bottom - top + 1) > SPREAD;
                if wide {
                    spread.push(index);
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
        for (&index, &(wide, left, right, top, bottom)) in top.iter().zip(&reach) {
            if wide {
                continue;
            }
            for row in top..=bottom {
                for column in left..=right {
                    let cell = row * columns + column;
                    tiled[fill[cell] as usize] = index;
                    fill[cell] += 1;
                }
            }
        }
        let rects = top
            .iter()
            .map(|&at| &items[at as usize])
            .map(|item| (item.placed.intersect(&item.own), item.order() / ORDER_STEP))
            .filter(|(rect, _)| !rect.is_empty())
            .collect();
        let mut tree = crate::bounds_tree::BoundsTree::packed(rects, |coordinate| coordinate.0);
        tree.freeze();
        let tree = std::rc::Rc::new(tree);
        Run { node, items, origin: area.origin, columns, rows, cells: counts, tiled, spread, live, tree }
    }

    /// Append every command of the run moved by `shift` and raised by
    /// `raised`, primitives with their own masks (a moved copy of it).
    fn expand(&self, shift: Point<ScaledPixels>, raised: DrawOrder, out: &mut Vec<PaintOperation>) {
        for item in &self.items {
            match &item.piece {
                Piece::Primitive(primitive) => {
                    let mut primitive = primitive.clone();
                    record::geometry::scrolled(&mut primitive, shift, item.pivot.as_deref());
                    *primitive.order_mut() += raised;
                    primitive.content_mask_mut().bounds = record::geometry::bounds(item.own, shift);
                    out.push(PaintOperation::Primitive(primitive));
                }
                _ => item.draw(shift, raised, Bounds::default(), out),
            }
        }
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
            if !self.live && item.placed.intersect(&item.own.intersect(&seen)).is_empty() {
                continue;
            }
            match item.piece {
                Piece::Layer(_) => {
                    let start = out.len();
                    for inner in &self.items[index as usize..(item.end as usize).max(index as usize + 1)] {
                        inner.draw(shift, raised, seen, out);
                    }
                    if self.live {
                        // Text layers order their contents as one unit. Their
                        // source bounds need the same live projection as glyphs.
                        let mut open: Vec<(usize, Option<Bounds<ScaledPixels>>)> = Vec::new();
                        for at in start..out.len() {
                            match &out[at] {
                                PaintOperation::StartLayer(..) => open.push((at, None)),
                                PaintOperation::Primitive(p) => {
                                    let bounds = placed_bounds(p).intersect(&p.content_mask().bounds);
                                    if !bounds.is_empty() {
                                        for (_, ink) in &mut open {
                                            *ink = Some(ink.map_or(bounds, |ink| ink.union(&bounds)));
                                        }
                                    }
                                }
                                PaintOperation::EndLayer => {
                                    if let Some((begin, ink)) = open.pop() {
                                        if let PaintOperation::StartLayer(bounds, _) = &mut out[begin] {
                                            *bounds = ink.unwrap_or_default();
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => item.draw(shift, raised, seen, out),
            }
        }
    }
}

impl Item {
    fn order(&self) -> DrawOrder {
        match &self.piece {
            Piece::Primitive(primitive) => primitive.order(),
            Piece::Layer(order) => *order,
            Piece::End => 0,
        }
    }

    /// Append this command moved by `shift` and raised by `raised`: a
    /// primitive only where `seen` (the clip in the run's coordinates) shows
    /// it, with (own ∩ seen) + shift as its mask; a layer's ends always.
    fn draw(&self, shift: Point<ScaledPixels>, raised: DrawOrder, seen: Bounds<ScaledPixels>, out: &mut Vec<PaintOperation>) {
        match &self.piece {
            Piece::Primitive(primitive) => {
                let mask = self.own.intersect(&seen);
                if mask.is_empty() {
                    return;
                }
                let mut primitive = primitive.clone();
                record::geometry::scrolled(&mut primitive, shift, self.pivot.as_deref());
                *primitive.order_mut() += raised;
                primitive.content_mask_mut().bounds = record::geometry::bounds(mask, shift);
                if placed_bounds(&primitive).intersect(&primitive.content_mask().bounds).is_empty() {
                    return;
                }
                out.push(PaintOperation::Primitive(primitive));
            }
            Piece::Layer(order) => {
                out.push(PaintOperation::StartLayer(record::geometry::bounds(self.placed, shift), order + raised));
            }
            Piece::End => out.push(PaintOperation::EndLayer),
        }
    }
}
