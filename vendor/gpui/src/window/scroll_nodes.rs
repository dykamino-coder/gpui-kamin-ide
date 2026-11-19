//! KaminIDE patch: scroll nodes of a frame, the spatial tree of scroll
//! containers a retained record can be replayed through.
//!
//! A scroll container moves its children by its scroll offset in prepaint
//! (`Interactivity::prepaint`, `with_element_offset`). The frame keeps one
//! record per container it prepainted: the node's identity (its scroll handle),
//! the enclosing node, the offset and the scrollport clip it was prepainted
//! with, and the prepaint and paint ranges of its children. A host that keeps a
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

/// One scroll container prepainted in a frame.
#[derive(Clone)]
pub(crate) struct ScrollNodeRecord {
    pub(crate) id: ScrollNodeId,
    /// Index of the enclosing node in the same frame.
    pub(crate) parent: Option<u32>,
    pub(crate) offset: Point<Pixels>,
    pub(crate) clip: Bounds<Pixels>,
    pub(crate) prepaint: Range<PrepaintStateIndex>,
    /// Empty until the node's children paint (`with_scroll_node_paint`).
    pub(crate) paint: Option<Range<PaintIndex>>,
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
        let index = self.next_frame.scroll_nodes.len() as u32;
        let start = self.prepaint_index();
        self.next_frame.scroll_nodes.push(ScrollNodeRecord {
            id,
            parent: self.next_frame.scroll_node_stack.last().copied(),
            offset,
            clip,
            prepaint: start.clone()..start,
            paint: None,
        });
        self.next_frame.scroll_node_stack.push(index);
        let result = f(self);
        self.next_frame.scroll_node_stack.pop();
        let end = self.prepaint_index();
        self.next_frame.scroll_nodes[index as usize].prepaint.end = end;
        result
    }

    /// Paint the children of a scroll container prepainted as node `id`
    /// in this frame; without that node `f` just runs.
    pub fn with_scroll_node_paint<R>(
        &mut self,
        id: ScrollNodeId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let start = self.paint_index();
        let result = f(self);
        let end = self.paint_index();
        if let Some(node) = self
            .next_frame
            .scroll_nodes
            .iter_mut()
            .rev()
            .find(|node| node.id == id && node.paint.is_none())
        {
            node.paint = Some(start..end);
        }
        result
    }

    /// `with_scroll_node_prepaint` when `node` (id and scrollport clip) is set.
    pub(crate) fn with_scroll_node_prepaint_if<R>(
        &mut self,
        node: Option<(ScrollNodeId, Bounds<Pixels>)>,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        match node {
            Some((id, clip)) => self.with_scroll_node_prepaint(id, clip, offset, f),
            None => f(self),
        }
    }

    /// `with_scroll_node_paint` when `node` is set.
    pub(crate) fn with_scroll_node_paint_if<R>(
        &mut self,
        node: Option<ScrollNodeId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        match node {
            Some(id) => self.with_scroll_node_paint(id, f),
            None => f(self),
        }
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
            range.start.scene_index >= paints.start.scene_index
                && range.end.scene_index <= paints.end.scene_index
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
}
