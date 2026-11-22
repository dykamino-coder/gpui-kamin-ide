//! KaminIDE patch: scroll nodes of a frame, the spatial tree of scroll
//! containers a retained record can be replayed through.
//!
//! A scroll container moves its children by its scroll offset in prepaint
//! (`Interactivity::prepaint`, `with_element_offset`). The frame keeps one
//! record per container it prepainted: the node's identity (its scroll handle),
//! the enclosing node, the offset and the scrollport clip it was prepainted
//! with, and the scene commands its children painted. A reused prepaint and
//! paint range copies the nodes in it (`reuse_scroll_nodes`). A host that keeps a
//! record across frames reads the nodes inside it (`retained_scroll_segments`)
//! to replay the children of a node at a new offset instead of building them
//! again. The tree is the same idea as cc's `ScrollTree` and WebRender's
//! spatial nodes: any scroll container is a node, nested ones form a chain.

use super::*;
use crate::ScrollHandle;

use crate::scene::{PaintOperation, scroll_marks::SceneSpace};

/// The scenes of the layers `operations` draw (`scene::layer`).
fn layer_spaces(operations: &[PaintOperation]) -> Vec<SceneSpace> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            PaintOperation::Layer(op) => op.cell.upgrade().map(|cell| cell.content.borrow().scene().scroll.space),
            _ => None,
        })
        .collect()
}

/// Identity of a scroll node across frames: the scroll handle's shared state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScrollNodeId(pub(crate) usize);

impl ScrollNodeId {
    /// The node of a scroll handle: stable while any clone of it lives.
    pub fn of(handle: &ScrollHandle) -> Self {
        Self(handle.node_identity())
    }

    /// The node of a host's anchor: stable while 	oken lives.
    pub fn of_token<T>(token: &Rc<T>) -> Self {
        Self(Rc::as_ptr(token) as *const () as usize)
    }

    /// A follower node of a scroll handle (its scrollbar thumb on `axis`):
    /// the handle's state is aligned, so the low bits name the follower.
    pub fn follower(handle: &ScrollHandle, axis: usize) -> Self {
        Self(handle.node_identity() | (1 + (axis & 1)))
    }
}

/// One scroll container prepainted in a frame (or copied into it with a
/// reused prepaint range).
#[derive(Clone)]
pub(crate) struct ScrollNodeRecord {
    pub(crate) id: ScrollNodeId,
    /// Index of the enclosing node in the same frame.
    pub(crate) parent: Option<u32>,
    pub(crate) offset: Point<Pixels>,
    /// Element offset the children were prepainted under: the offset at the
    /// node plus its own (`scroll_node_children_offset`).
    pub(crate) children_offset: Point<Pixels>,
    /// Real clip chain of the children: the enclosing node's clip and the
    /// content mask at the node (`chain`).
    pub(crate) clip: Bounds<Pixels>,
    /// Content mask at the node: under an enclosing node's interest rect,
    /// not its real clip, so the clip follows a replay of that node.
    pub(crate) chain: Bounds<Pixels>,
    /// Where the children's content is recorded: the clip widened by the
    /// window's interest margin (`set_scroll_interest_margin`). A replay
    /// may show the children only inside it.
    pub(crate) interest: Bounds<Pixels>,
    /// An anchor (`with_scroll_anchor_prepaint`): its mask is fixed to the
    /// viewport, not to the enclosing node's content.
    pub(crate) anchored: bool,
    /// Where the node's offset is read when a replay plans: the container's
    /// scroll handle, or the host's function for an anchor.
    pub(crate) source: NodeSource,
    /// Scene commands of the node's children; empty until they paint.
    pub(crate) paint: Option<Range<usize>>,
    /// The scene `paint` counts in: a layer's while one records.
    pub(crate) painted_in: Option<SceneSpace>,
    /// Paint of the children deferred out of `paint` and run later under the
    /// node again (`PaintCtx` replays): more ranges of the same scene.
    pub(crate) more: Vec<Range<usize>>,
    /// Deferred paint of the children landed in another scene than `paint`:
    /// a scrolled replay cannot move it.
    pub(crate) torn: bool,
    /// Paired with a paint that has not ended yet.
    pub(crate) pending: bool,
}

impl ScrollNodeRecord {
    /// Take a landed paint range: the node's `paint` or one of `more`.
    pub(crate) fn land(&mut self, range: Range<usize>, space: SceneSpace, main: bool) {
        if main {
            (self.paint, self.painted_in) = (Some(range), Some(space));
        } else {
            self.more.push(range);
        }
    }

    /// All its paint ranges.
    pub(crate) fn ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.paint.iter().cloned().chain(self.more.iter().cloned())
    }
}

mod source;
pub(crate) use source::NodeSource;

/// Pairing of scroll node paints with their prepaints in one frame.
#[derive(Default)]
pub(crate) struct PaintPairing {
    /// Nodes before it are paired or skipped (`begin_scroll_node_paint`).
    cursor: usize,
    /// Copied nodes waiting for the paint range that holds them: index and
    /// scene range in the rendered frame (`reuse_scroll_nodes`).
    /// The last field: whether the range is the node's `paint` (else one of `more`).
    pub(crate) landing: Vec<(u32, Range<usize>, SceneSpace, bool)>,
    /// Content mask depth under each open prepaint node.
    pub(crate) depths: Vec<usize>,
    /// Open paint nodes and the content mask depth under each.
    pub(crate) open: Vec<(u32, usize)>,
}

impl PaintPairing {
    pub(crate) fn clear(&mut self) {
        self.cursor = 0;
        self.landing.clear();
        self.depths.clear();
        self.open.clear();
    }
}

/// A scroll node inside a retained record, as the record's frame saw it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollSegment {
    /// The scroll container's node.
    pub id: ScrollNodeId,
    /// Position of the enclosing segment in the same list, outermost first.
    pub parent: Option<usize>,
    /// Scroll offset the children were prepainted with.
    pub offset: Point<Pixels>,
    /// Scrollport clip (window pixels) the children were prepainted under.
    pub clip: Bounds<Pixels>,
}

impl Window {
    /// Prepaint the children of a scroll container as a scroll node: `f` runs
    /// with the node open, so nodes it opens are its children.
    pub fn with_scroll_node_prepaint<R>(
        &mut self,
        handle: &ScrollHandle,
        clip: Bounds<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let open = self.begin_scroll_node_prepaint(Some((handle, clip)), handle.offset());
        let result = f(self);
        self.end_scroll_node_prepaint(open);
        result
    }

    /// Paint the children of a scroll container prepainted as node `id`
    /// in this frame; without that node `f` just runs.
    pub fn with_scroll_node_paint<R>(
        &mut self,
        id: ScrollNodeId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let open = self.begin_scroll_node_paint(Some(id));
        let result = f(self);
        self.end_scroll_node_paint(open);
        result
    }

    /// Open the scroll node of a container whose children prepaint next, when
    /// `node` (id and scrollport clip) is set. Begin/end calls instead of a
    /// closure keep the element's stack frame as it was: deep documents
    /// prepaint one Div per level (`depth_tests`).
    pub(crate) fn begin_scroll_node_prepaint(
        &mut self,
        node: Option<(&ScrollHandle, Bounds<Pixels>)>,
        offset: Point<Pixels>,
    ) -> Option<u32> {
        let (handle, chain) = node?;
        let source = NodeSource::Handle(handle.clone());
        Some(self.open_scroll_node(ScrollNodeId::of(handle), chain, offset, source))
    }

    /// Open a scroll anchor: a node whose children stay where the host puts
    /// them while enclosing nodes scroll (a fixed or sticky box), offset
    /// against the enclosing node by `offset` (for a fixed box, minus the
    /// enclosing scroll offset). Its mask does not move with that node.
    #[track_caller]
    pub fn with_scroll_anchor_prepaint<R>(
        &mut self,
        id: ScrollNodeId,
        offset: Rc<dyn Fn() -> Point<Pixels>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let chain = self.content_mask().bounds;
        let now = offset();
        let index = self.open_scroll_node(id, chain, now, NodeSource::Anchor(offset));
        // KaminIDE patch (DevTools `why`): the anchor, where its host opened it.
        crate::dirty_sources::trace::push_with("scroll_node.open_anchor", core::panic::Location::caller(), || {
            let node = &self.next_frame.scroll_nodes[index as usize];
            let tail = self.element_id_stack.iter().rev().take(3).map(|e| format!("{e:?}")).collect::<Vec<_>>();
            format!("node {index} id {} chain {:?} clip {:?} element ..{}", id.0, node.chain, node.clip, tail.join(" < "))
        });
        let result = f(self);
        self.end_scroll_node_prepaint(Some(0));
        result
    }

    /// Open a follower: a node whose children move by `at(change)` as
    /// another node's offset changes by `change` since now — of `on`, a
    /// scroller's handle (its scrollbar thumb), or else of the innermost open
    /// node (a sticky box under its scroller). Unlike an anchor its mask
    /// moves with the enclosing node. The geometry `at` reads is the
    /// host's, recorded now: a scrolled replay moves the children by its
    /// change, as cc's compositor places sticky boxes and scrollbar thumbs
    /// from the scroll offset alone. Without a node to follow, `f` just runs.
    pub fn with_scroll_follower_prepaint<R>(
        &mut self,
        id: ScrollNodeId,
        on: Option<&ScrollHandle>,
        at: Rc<dyn Fn(Point<Pixels>) -> Point<Pixels>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let on = match on {
            Some(handle) => NodeSource::Handle(handle.clone()),
            None => match self.next_frame.scroll_node_stack.last() {
                Some(&top) => self.next_frame.scroll_nodes[top as usize].source.clone(),
                None => return f(self),
            },
        };
        let (chain, base) = (self.content_mask().bounds, on.offset());
        let now = at(Point::default());
        self.open_scroll_node(id, chain, now, NodeSource::Follow { on: Box::new(on), base, at });
        let result = f(self);
        self.end_scroll_node_prepaint(Some(0));
        result
    }

    /// The element offset under which the children of the open scroll node
    /// of `handle` prepaint (the innermost one, when nested copies are open),
    /// or `None` outside it. A box laid out in that node's content but
    /// prepainted elsewhere (painted at its source position, CSS 2.1
    /// Appendix E) takes its coordinates from here: that is the space of its
    /// containing block, as Blink takes a positioned box's transform from
    /// its containing block's context and not from its tree position.
    pub fn scroll_node_children_offset(&self, handle: &ScrollHandle) -> Option<Point<Pixels>> {
        let id = ScrollNodeId::of(handle);
        let nodes = &self.next_frame.scroll_nodes;
        self.next_frame
            .scroll_node_stack
            .iter()
            .rev()
            .map(|&index| &nodes[index as usize])
            .find(|node| node.id == id)
            .map(|node| node.children_offset)
    }

    /// The visible part of what the innermost open prepaint node shows:
    /// the content mask, which under a node is its interest rect, clipped
    /// by the node's real clip chain (a sticky box's scrollport).
    pub fn scroll_visible_mask(&self) -> Bounds<Pixels> {
        let mask = self.content_mask().bounds;
        match self.next_frame.scroll_node_stack.last() {
            Some(&top) => mask.intersect(&self.next_frame.scroll_nodes[top as usize].clip),
            None => mask,
        }
    }

    pub(super) fn open_scroll_node(&mut self, id: ScrollNodeId, chain: Bounds<Pixels>, offset: Point<Pixels>, source: NodeSource) -> u32 {
        let anchored = matches!(source, NodeSource::Anchor(_) | NodeSource::Viewport { anchored: true });
        let index = self.next_frame.scroll_nodes.len() as u32;
        let parent = self.next_frame.scroll_node_stack.last().copied();
        let clip = match parent {
            Some(parent) => self.next_frame.scroll_nodes[parent as usize].clip.intersect(&chain),
            None => chain,
        };
        let interest = if matches!(source, NodeSource::Viewport { .. }) {
            Bounds { origin: Point::default(), size: self.viewport_size() }
        } else {
            clip.dilate(self.scroll_interest_margin)
        };
        // Read the stack directly instead of `element_offset()`: opening a
        // node is not tied to a draw phase (records are built outside one),
        // and outside prepaint the stack is empty, so the base is zero.
        let base = self.element_offset_stack.last().copied().unwrap_or_default();
        let children_offset = base + offset;
        self.next_frame.scroll_nodes.push(ScrollNodeRecord {
            id,
            parent,
            offset,
            children_offset,
            clip,
            chain,
            interest,
            anchored,
            source,
            paint: None,
            painted_in: None,
            more: Vec::new(),
            torn: false,
            pending: false,
        });
        self.next_frame.scroll_node_stack.push(index);
        self.open_interest(interest, false);
        index
    }

    /// Close a node opened by `begin_scroll_node_prepaint`.
    pub(crate) fn end_scroll_node_prepaint(&mut self, open: Option<u32>) {
        if open.is_some() {
            self.next_frame.scroll_node_stack.pop();
            self.close_interest(false);
        }
    }

    /// Start the paint of a container's children as node `id`: pair it with
    /// the first node of that id prepainted at or after the pairing cursor.
    /// Paint visits containers in prepaint order, so the cursor only moves
    /// forward and pairing all nodes of a frame is linear; a node a reused
    /// paint range already placed is never paired again.
    #[track_caller]
    pub(crate) fn begin_scroll_node_paint(&mut self, id: Option<ScrollNodeId>) -> Option<u32> {
        let id = id?;
        let frame = &mut self.next_frame;
        let from = frame.scroll_paint.cursor;
        let free = |node: &ScrollNodeRecord| node.id == id && node.paint.is_none() && !node.pending;
        let found = frame.scroll_nodes[from..]
            .iter()
            .position(free)
            .map(|at| from + at)
            // Out of order (a container painted outside its prepaint order).
            .or_else(|| frame.scroll_nodes[..from].iter().rposition(free));
        if crate::dirty_sources::trace::enabled() {
            trace_pairing(&self.next_frame, &self.element_id_stack, id, from, found);
        }
        let index = found?;
        let frame = &mut self.next_frame;
        frame.scroll_paint.cursor = frame.scroll_paint.cursor.max(index + 1);
        let start = frame.scene.paint_operations.len();
        frame.scroll_nodes[index].paint = Some(start..start);
        frame.scroll_nodes[index].painted_in = Some(frame.scene.scroll.space);
        frame.scroll_nodes[index].pending = true;
        // Inside an open layer or depth context its commands take orders a
        // moved copy cannot give again (`Scene::scrollable` sees only its range).
        frame.scroll_nodes[index].torn |= !frame.scene.at_root();
        let interest = frame.scroll_nodes[index].interest;
        self.open_interest(interest, true);
        let depth = self.content_mask_stack.len() - 1;
        self.next_frame.scroll_paint.open.push((index as u32, depth));
        Some(index as u32)
    }

    /// Enter or leave (alternating, as a `PaintCtx` replay swaps its state in
    /// and out) the scroll node `node` that painted when the context was
    /// captured: paint deferred out of a node's children (positioned and
    /// inline jobs) runs under it again, its primitives marked under it, and
    /// what it painted becomes one more range of the node. A node whose paint is still open holds
    /// that range already; one in another scene is torn (`plan_scrolled_replay` then builds), but a
    /// group's scene: it is copied with its label, and its commands checked (`group_replay`).
    pub(crate) fn reenter_scroll_node(
        &mut self,
        node: Option<(u32, usize)>,
        reentry: &mut Option<(usize, SceneSpace)>,
        swapped: &mut bool,
    ) {
        let Some(node) = node else { return };
        let frame = &mut self.next_frame;
        let (ops, space) = (frame.scene.paint_operations.len(), frame.scene.scroll.space);
        *swapped = !*swapped;
        if *swapped {
            frame.scroll_paint.open.push(node);
            *reentry = Some((ops, space));
            frame.scroll_nodes[node.0 as usize].torn |= !frame.scene.at_root();
            return;
        }
        frame.scroll_paint.open.pop();
        let Some((start, began)) = reentry.take() else { return };
        let record = &mut frame.scroll_nodes[node.0 as usize];
        if ops <= start || record.pending {
            return;
        }
        if record.painted_in == Some(space) && began == space {
            record.more.push(start..ops);
        } else if !(frame.scene.in_group && began == space) {
            record.torn = true;
        }
    }

    /// End the paint started by `begin_scroll_node_paint`.
    pub(crate) fn end_scroll_node_paint(&mut self, open: Option<u32>) {
        let Some(index) = open else { return };
        self.close_interest(true);
        let frame = &mut self.next_frame;
        let end = frame.scene.paint_operations.len();
        let node = &mut frame.scroll_nodes[index as usize];
        node.pending = false;
        if let Some(paint) = &mut node.paint {
            paint.end = end;
        }
    }

    /// `reuse_prepaint` of `range`: copy the rendered frame's nodes in it.
    /// Parents inside the range are renumbered; a parent outside is the node
    /// open now. Paint ranges land with the paint range that holds them
    /// (`land_scroll_nodes`); until then a copy is not paired or listed.
    pub(crate) fn reuse_scroll_nodes(&mut self, range: &Range<PrepaintStateIndex>) {
        // Hitbox marks, renumbered as `reuse_prepaint` appends the hitboxes.
        let hitboxes = range.start.hitboxes_index as u32..range.end.hitboxes_index as u32;
        let landed = self.next_frame.hitboxes.len() as u32;
        let marks = &self.rendered_frame.scroll_hitboxes;
        let from = marks.partition_point(|mark| mark.index < hitboxes.start);
        let to = marks.partition_point(|mark| mark.index < hitboxes.end);
        self.next_frame.scroll_hitboxes.extend(marks[from..to].iter().map(|mark| super::scroll_masks::HitboxMark {
            index: landed + mark.index - hitboxes.start,
            ..*mark
        }));
        let old = range.start.scroll_nodes_index..range.end.scroll_nodes_index;
        if old.is_empty() {
            return;
        }
        let first = self.next_frame.scroll_nodes.len();
        let open = self.next_frame.scroll_node_stack.last().copied();
        let new_index = |old_index: usize| (first + old_index - old.start) as u32;
        for (at, node) in self.rendered_frame.scroll_nodes[old.clone()].iter().enumerate() {
            let parent = match node.parent {
                Some(parent) if old.contains(&(parent as usize)) => Some(new_index(parent as usize)),
                _ => open,
            };
            self.next_frame.scroll_nodes.push(ScrollNodeRecord {
                parent,
                paint: None,
                painted_in: None,
                more: Vec::new(),
                pending: false,
                ..node.clone()
            });
            if let (Some(paint), Some(space)) = (&node.paint, node.painted_in) {
                let index = new_index(old.start + at);
                let landing = &mut self.next_frame.scroll_paint.landing;
                landing.push((index, paint.clone(), space, true));
                landing.extend(node.more.iter().map(|more| (index, more.clone(), space, false)));
            }
        }
    }

    /// `reuse_paint` of `scenes` of the scene in space `from` landed at
    /// `landed`: the copied nodes and dependents painted in it take the moved
    /// range. A copy that was not one-to-one leaves them unpainted (not
    /// listed as segments). Ranges in the scene of a layer the copy draws
    /// (the layer's one command) stay: the layer holds them as they were.
    pub(crate) fn land_scroll_paint(&mut self, scenes: Range<usize>, landed: usize, from: SceneSpace) {
        let frame = &mut self.next_frame;
        let foreign = |space: SceneSpace| space != from;
        let dependents = &self.rendered_frame.scroll_dependents;
        if frame.scroll_paint.landing.is_empty() && dependents.is_empty() {
            return;
        }
        let exact = frame.scene.paint_operations.len() - landed == scenes.len();
        let here = frame.scene.scroll.space;
        let layers: Vec<SceneSpace> = if frame.scroll_paint.landing.iter().any(|entry| foreign(entry.2))
            || dependents.iter().any(|entry| foreign(entry.2))
        {
            layer_spaces(&frame.scene.paint_operations[landed..])
        } else {
            Vec::new()
        };
        let inside = |paint: &Range<usize>| paint.start >= scenes.start && paint.end <= scenes.end;
        let map = |at: usize| landed + at - scenes.start;
        let nodes = &mut frame.scroll_nodes;
        frame.scroll_paint.landing.retain(|(index, paint, space, main)| {
            let node = &mut nodes[*index as usize];
            if layers.contains(space) {
                node.land(paint.clone(), *space, *main);
                return false;
            }
            if foreign(*space) || !inside(paint) {
                return true;
            }
            if exact {
                node.land(map(paint.start)..map(paint.end), here, *main);
            }
            false
        });
        let landed: Vec<_> = dependents
            .iter()
            .filter_map(|&(id, at, space)| {
                if layers.contains(&space) {
                    Some((id, at, space))
                } else {
                    (exact && !foreign(space) && scenes.contains(&at)).then(|| (id, map(at), here))
                }
            })
            .collect();
        frame.scroll_dependents.extend(landed);
    }
    /// The scroll nodes of the rendered frame inside a record made in it:
    /// both ranges of each node lie within the record's, outermost first, each
    /// with the position of its enclosing segment when that one is inside too.
    pub fn retained_scroll_segments(
        &self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
    ) -> Vec<ScrollSegment> {
        segments(&self.rendered_frame.scroll_nodes, prepaint, paint)
    }
}

pub(crate) fn segments(
    nodes: &[ScrollNodeRecord],
    prepaint: &RetainedPrepaint,
    paint: &RetainedPaint,
) -> Vec<ScrollSegment> {
    // A record's prepaint range counts the nodes prepainted inside it, and the
    // node's paint must lie inside its paint range too.
    let (prepaints, paints) = (prepaint.range(), paint.range());
    let nodes = nodes
        .get(prepaints.start.scroll_nodes_index..prepaints.end.scroll_nodes_index)
        .unwrap_or_default();
    let first = prepaints.start.scroll_nodes_index;
    let inside = |node: &ScrollNodeRecord| {
        node.paint.as_ref().is_some_and(|range| {
            range.start >= paints.start.scene_index && range.end <= paints.end.scene_index
        })
    };
    let mut positions = FxHashMap::<u32, usize>::default();
    let mut out = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        if !inside(node) {
            continue;
        }
        positions.insert((first + index) as u32, out.len());
        out.push(ScrollSegment {
            id: node.id,
            parent: node.parent.and_then(|parent| positions.get(&parent).copied()),
            offset: node.offset,
            clip: node.clip,
        });
    }
    out
}

/// KaminIDE patch (DevTools `why`): a paint pairing with an anchor's id or
/// one that found no node: the node found, the cursor and, on a miss, what
/// the nodes of that id are (paired, pending), so a reader tells a paint
/// that came with another id from one that paired with a foreign node.
#[track_caller]
fn trace_pairing(frame: &Frame, elements: &[ElementId], id: ScrollNodeId, cursor: usize, found: Option<usize>) {
    let same: Vec<(usize, &ScrollNodeRecord)> = frame.scroll_nodes.iter().enumerate().filter(|(_, n)| n.id == id).collect();
    let anchor = same.iter().any(|(_, n)| n.anchored);
    if found.is_some() && !anchor {
        return;
    }
    let site = core::panic::Location::caller();
    let kind = if found.is_some() { "scroll_node.paint_paired" } else { "scroll_node.paint_unpaired" };
    crate::dirty_sources::trace::push_with(kind, site, || {
        let nodes: Vec<String> = same
            .iter()
            .map(|(at, n)| {
                let state = match (n.paint.is_some(), n.pending) {
                    (_, true) => "pending",
                    (true, false) => "paired",
                    (false, false) => "free",
                };
                format!("{at}:{state}{}", if n.anchored { ":anchor" } else { "" })
            })
            .collect();
        let tail = elements.iter().rev().take(3).map(|e| format!("{e:?}")).collect::<Vec<_>>();
        format!(
            "id {} cursor {cursor} found {found:?} nodes of the id [{}] of {} element ..{}",
            id.0,
            nodes.join(" "),
            frame.scroll_nodes.len(),
            tail.join(" < ")
        )
    });
}

impl Window {
    /// KaminIDE patch (DevTools `why`): anchors of the frame just drawn whose
    /// children never painted (`paint` none): prepaint opened them, paint did
    /// not pair them.
    #[track_caller]
    pub(crate) fn trace_unpainted_anchors(&self) {
        if !crate::dirty_sources::trace::enabled() {
            return;
        }
        let site = core::panic::Location::caller();
        for (at, node) in self.rendered_frame.scroll_nodes.iter().enumerate() {
            if node.anchored && node.paint.is_none() {
                crate::dirty_sources::trace::push_with("scroll_node.anchor_unpainted", site, || {
                    format!("node {at} id {} chain {:?} clip {:?} parent {:?}", node.id.0, node.chain, node.clip, node.parent)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestAppContext, div, px, size};

    struct Nested;

    impl Render for Nested {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// Two nested scroll nodes and one outside the record: the record's
    /// segments are the two, outermost first, the inner one under the outer.
    #[crate::test]
    fn a_record_lists_its_nested_scroll_nodes(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Nested);
        window
            .update(cx, |_, window, cx| {
                let (outer, inner, other) =
                    (ScrollHandle::new(), ScrollHandle::new(), ScrollHandle::new());
                let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(50.)));
                let small = Bounds::new(point(px(10.), px(10.)), size(px(20.), px(20.)));
                { other.set_offset(point(px(0.), px(-5.))); &mut *window }.with_scroll_node_prepaint(&other, clip, |_| {});
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    { outer.set_offset(point(px(0.), px(-40.))); &mut *window }.with_scroll_node_prepaint(&outer, clip, |window| {
                        { inner.set_offset(point(px(-3.), px(0.))); &mut *window }.with_scroll_node_prepaint(&inner, small, |_| {});
                    });
                });
                window.with_scroll_node_paint(ScrollNodeId::of(&other), |_| {});
                let (_, paint) = window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(ScrollNodeId::of(&outer), |window| {
                        window.with_scroll_node_paint(ScrollNodeId::of(&inner), |_| {});
                    });
                });
                let found = segments(&window.next_frame.scroll_nodes, &prepaint, &paint);
                assert_eq!(
                    found,
                    vec![
                        ScrollSegment {
                            id: ScrollNodeId::of(&outer),
                            parent: None,
                            offset: point(px(0.), px(-40.)),
                            clip,
                        },
                        ScrollSegment {
                            id: ScrollNodeId::of(&inner),
                            parent: Some(0),
                            offset: point(px(-3.), px(0.)),
                            clip: small,
                        },
                    ]
                );
                let _ = cx;
            })
            .unwrap();
    }

    fn quad(window: &mut Window, y: f32) {
        let bounds = Bounds::new(point(px(0.), px(y)), size(px(10.), px(10.))).scale(1.);
        window.next_frame.scene.insert_primitive(crate::Quad {
            bounds,
            content_mask: ContentMask { bounds },
            ..Default::default()
        });
    }

    /// A record replayed into the next frame (`replay_retained_prepaint`,
    /// `replay_retained_paint`) lists the same segments: the nodes in it are
    /// copied, renumbered, and take the moved scene range.
    #[crate::test]
    fn a_replayed_record_keeps_its_scroll_nodes(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Nested);
        window
            .update(cx, |_, window, _| {
                let (outer, inner, other) =
                    (ScrollHandle::new(), ScrollHandle::new(), ScrollHandle::new());
                let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(50.)));
                let id = ScrollNodeId::of;
                { other.set_offset(point(px(0.), px(-5.))); &mut *window }.with_scroll_node_prepaint(&other, clip, |_| {});
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    { outer.set_offset(point(px(0.), px(-40.))); &mut *window }.with_scroll_node_prepaint(&outer, clip, |window| {
                        { inner.set_offset(point(px(-3.), px(0.))); &mut *window }.with_scroll_node_prepaint(&inner, clip, |_| {});
                    });
                });
                window.with_scroll_node_paint(id(&other), |window| quad(window, 0.));
                let (_, paint) = window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(id(&outer), |window| {
                        quad(window, 1.);
                        window.with_scroll_node_paint(id(&inner), |window| quad(window, 2.));
                    });
                });
                let recorded = segments(&window.next_frame.scroll_nodes, &prepaint, &paint);
                assert_eq!(recorded.len(), 2);
                std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
                window.next_frame.clear();
                // Something painted before the replay moves the scene range.
                quad(window, 3.);
                let prepaint = window.replay_retained_prepaint(&prepaint);
                let paint = window.replay_retained_paint(&paint);
                assert_eq!(segments(&window.next_frame.scroll_nodes, &prepaint, &paint), recorded);
                let inner = &window.next_frame.scroll_nodes[1];
                assert_eq!((inner.parent, inner.paint.clone()), (Some(0), Some(2..3)));
            })
            .unwrap();
    }

    /// Siblings of one id pair their paints in prepaint order, and pairing
    /// skips nodes already painted instead of searching from the end.
    #[crate::test]
    fn sibling_nodes_pair_their_paints_in_order(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Nested);
        window
            .update(cx, |_, window, _| {
                let handle = ScrollHandle::new();
                let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(50.)));
                for y in [-1., -2.] {
                    { handle.set_offset(point(px(0.), px(y))); &mut *window }.with_scroll_node_prepaint(&handle, clip, |_| {});
                }
                window.with_scroll_node_paint(ScrollNodeId::of(&handle), |window| quad(window, 0.));
                let painted = |window: &Window| {
                    window.next_frame.scroll_nodes.iter().map(|node| node.paint.clone()).collect::<Vec<_>>()
                };
                assert_eq!(painted(window), vec![Some(0..1), None]);
                window.with_scroll_node_paint(ScrollNodeId::of(&handle), |_| {});
                assert_eq!(painted(window), vec![Some(0..1), Some(1..1)]);
                assert_eq!(window.next_frame.scroll_paint.cursor, 2);
            })
            .unwrap();
    }

    /// While DevTools trace, an anchor's opening and the pairing of paints
    /// with its id are recorded with what they saw; a paint of an id no
    /// node has records why it found none.
    #[crate::test]
    fn anchor_pairing_is_traced_while_a_reader_lives(cx: &mut TestAppContext) {
        use crate::dirty_sources::trace;
        let window = cx.add_window(|_, _| Nested);
        let trace = trace::Trace::start();
        trace::take();
        window
            .update(cx, |_, window, _| {
                let token = Rc::new(());
                let id = ScrollNodeId::of_token(&token);
                window.invalidator.set_phase(DrawPhase::Prepaint);
                window.with_scroll_anchor_prepaint(id, Rc::new(|| point(px(0.), px(0.))), |_| {});
                window.invalidator.set_phase(DrawPhase::Paint);
                window.with_scroll_node_paint(ScrollNodeId::of_token(&Rc::new(())), |_| {});
                window.with_scroll_node_paint(id, |_| {});
            })
            .unwrap();
        let (events, _) = trace::take();
        drop(trace);
        let kinds: Vec<_> = events.iter().map(|e| e.kind).filter(|k| k.starts_with("scroll_node.")).collect();
        assert_eq!(kinds, ["scroll_node.open_anchor", "scroll_node.paint_unpaired", "scroll_node.paint_paired"]);
        let unpaired = events.iter().find(|e| e.kind == "scroll_node.paint_unpaired").unwrap();
        assert!(unpaired.detail.contains("found None nodes of the id []"), "{}", unpaired.detail);
        let paired = events.iter().find(|e| e.kind == "scroll_node.paint_paired").unwrap();
        assert!(paired.detail.contains("found Some(0) nodes of the id [0:free:anchor]"), "{}", paired.detail);
    }
}