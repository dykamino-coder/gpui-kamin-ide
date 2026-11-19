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

/// Identity of a scroll node across frames: the scroll handle's shared state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScrollNodeId(pub(crate) usize);

impl ScrollNodeId {
    /// The node of a scroll handle: stable while any clone of it lives.
    pub fn of(handle: &ScrollHandle) -> Self {
        Self(handle.node_identity())
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
    /// Scene commands of the node's children; empty until they paint.
    pub(crate) paint: Option<Range<usize>>,
    /// Paired with a paint that has not ended yet.
    pub(crate) pending: bool,
}

/// Pairing of scroll node paints with their prepaints in one frame.
#[derive(Default)]
pub(crate) struct PaintPairing {
    /// Nodes before it are paired or skipped (`begin_scroll_node_paint`).
    cursor: usize,
    /// Copied nodes waiting for the paint range that holds them: index and
    /// scene range in the rendered frame (`reuse_scroll_nodes`).
    pub(crate) landing: Vec<(u32, Range<usize>)>,
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
        id: ScrollNodeId,
        clip: Bounds<Pixels>,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let open = self.begin_scroll_node_prepaint(Some((id, clip)), offset);
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
        node: Option<(ScrollNodeId, Bounds<Pixels>)>,
        offset: Point<Pixels>,
    ) -> Option<u32> {
        let (id, chain) = node?;
        Some(self.open_scroll_node(id, chain, offset, false))
    }

    /// Open a scroll anchor: a node whose children stay where the host puts
    /// them while enclosing nodes scroll (a fixed or sticky box), offset
    /// against the enclosing node by `offset` (for a fixed box, minus the
    /// enclosing scroll offset). Its mask does not move with that node.
    pub fn with_scroll_anchor_prepaint<R>(
        &mut self,
        id: ScrollNodeId,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let chain = self.content_mask().bounds;
        self.open_scroll_node(id, chain, offset, true);
        let result = f(self);
        self.end_scroll_node_prepaint(Some(0));
        result
    }

    fn open_scroll_node(&mut self, id: ScrollNodeId, chain: Bounds<Pixels>, offset: Point<Pixels>, anchored: bool) -> u32 {
        let index = self.next_frame.scroll_nodes.len() as u32;
        let parent = self.next_frame.scroll_node_stack.last().copied();
        let clip = match parent {
            Some(parent) => self.next_frame.scroll_nodes[parent as usize].clip.intersect(&chain),
            None => chain,
        };
        let interest = clip.dilate(self.scroll_interest_margin);
        self.next_frame.scroll_nodes.push(ScrollNodeRecord {
            id,
            parent,
            offset,
            clip,
            chain,
            interest,
            anchored,
            paint: None,
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
    pub(crate) fn begin_scroll_node_paint(&mut self, id: Option<ScrollNodeId>) -> Option<u32> {
        let id = id?;
        let frame = &mut self.next_frame;
        let from = frame.scroll_paint.cursor;
        let free = |node: &ScrollNodeRecord| node.id == id && node.paint.is_none() && !node.pending;
        let index = frame.scroll_nodes[from..]
            .iter()
            .position(free)
            .map(|at| from + at)
            // Out of order (a container painted outside its prepaint order).
            .or_else(|| frame.scroll_nodes[..from].iter().rposition(free))?;
        frame.scroll_paint.cursor = frame.scroll_paint.cursor.max(index + 1);
        let start = frame.scene.paint_operations.len();
        frame.scroll_nodes[index].paint = Some(start..start);
        frame.scroll_nodes[index].pending = true;
        let interest = frame.scroll_nodes[index].interest;
        self.open_interest(interest, true);
        let depth = self.content_mask_stack.len() - 1;
        self.next_frame.scroll_paint.open.push((index as u32, depth));
        Some(index as u32)
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
                pending: false,
                ..node.clone()
            });
            if let Some(paint) = &node.paint {
                self.next_frame.scroll_paint.landing.push((new_index(old.start + at), paint.clone()));
            }
        }
    }

    /// `reuse_paint` of the rendered scene's `scenes` landed at `landed`: the
    /// copied nodes painted in it take the moved range. A copy that was not
    /// one-to-one leaves them unpainted (not listed as segments).
    pub(crate) fn land_scroll_nodes(&mut self, scenes: Range<usize>, landed: usize) {
        let frame = &mut self.next_frame;
        if frame.scroll_paint.landing.is_empty() {
            return;
        }
        let exact = frame.scene.paint_operations.len() - landed == scenes.len();
        let nodes = &mut frame.scroll_nodes;
        frame.scroll_paint.landing.retain(|(index, paint)| {
            if paint.start < scenes.start || paint.end > scenes.end {
                return true;
            }
            if exact {
                let at = landed + paint.start - scenes.start;
                nodes[*index as usize].paint = Some(at..at + paint.len());
            }
            false
        });
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
                window.with_scroll_node_prepaint(ScrollNodeId::of(&other), clip, point(px(0.), px(-5.)), |_| {});
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    window.with_scroll_node_prepaint(ScrollNodeId::of(&outer), clip, point(px(0.), px(-40.)), |window| {
                        window.with_scroll_node_prepaint(ScrollNodeId::of(&inner), small, point(px(-3.), px(0.)), |_| {});
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
                window.with_scroll_node_prepaint(id(&other), clip, point(px(0.), px(-5.)), |_| {});
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    window.with_scroll_node_prepaint(id(&outer), clip, point(px(0.), px(-40.)), |window| {
                        window.with_scroll_node_prepaint(id(&inner), clip, point(px(-3.), px(0.)), |_| {});
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
                    window.with_scroll_node_prepaint(ScrollNodeId::of(&handle), clip, point(px(0.), px(y)), |_| {});
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
}