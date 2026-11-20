//! KaminIDE patch: masks under scroll nodes (`scroll_nodes`).
//!
//! The children of a scroll node are prepainted and painted under the node's
//! interest rect instead of its clip, so their own masks (`scene::scroll_marks`)
//! cover what a replay at another offset may show. Each primitive and hitbox
//! is drawn and hit-tested with own mask ∩ the node's real clip chain.

use super::*;
use crate::ScrollHandle;
use crate::scene::{Primitive, scroll_marks::SceneSpace};

/// The scroll node and own mask of a hitbox inserted under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HitboxMark {
    /// Index of the hitbox in the frame's `hitboxes`.
    pub(crate) index: u32,
    pub(crate) node: ScrollNodeId,
    pub(crate) own: Bounds<Pixels>,
}

impl Window {
    /// Run `f` with the scroll nodes it opens recording `margin` past their
    /// clip (window pixels, each side): a host's subtree that replays scrolled
    /// (`plan_scrolled_replay`). Zero, the default, records what is visible.
    pub fn with_scroll_interest_margin<R>(&mut self, margin: Pixels, f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.scroll_interest_margin, margin);
        let result = f(self);
        self.scroll_interest_margin = outer;
        result
    }

    /// The margin `with_scroll_interest_margin` set for what runs now.
    pub fn scroll_interest_margin(&self) -> Pixels {
        self.scroll_interest_margin
    }

    /// Push a node's interest rect as the content mask of its children.
    pub(super) fn open_interest(&mut self, interest: Bounds<Pixels>, paint: bool) {
        let depth = self.content_mask_stack.len();
        self.content_mask_stack.push(ContentMask { bounds: interest });
        if !paint {
            self.next_frame.scroll_paint.depths.push(depth);
        }
    }

    /// Pop the mask `open_interest` pushed for the innermost prepaint or
    /// paint node.
    pub(super) fn close_interest(&mut self, paint: bool) {
        let pairing = &mut self.next_frame.scroll_paint;
        let depth = if paint { pairing.open.pop().map(|(_, depth)| depth) } else { pairing.depths.pop() };
        if let Some(depth) = depth {
            self.content_mask_stack.truncate(depth);
        }
    }

    /// Insert a primitive into the next frame's scene, under the innermost
    /// open paint node when there is one.
    pub(crate) fn insert_scene_primitive(&mut self, primitive: impl Into<Primitive>) {
        let scale = self.scale_factor();
        let frame = &mut self.next_frame;
        let open = frame.scroll_paint.open.last().map(|&(index, _)| {
            let node = &frame.scroll_nodes[index as usize];
            (node.id, node.clip.scale(scale))
        });
        frame.scene.scroll.open = open;
        frame.scene.insert_primitive(primitive);
        frame.scene.scroll.open = None;
    }

    /// The mask a hitbox inserted now is hit-tested with, and its mark when
    /// it lies under a prepaint node: own mask ∩ the node's clip.
    pub(crate) fn scroll_hitbox_mask(
        &self,
        mask: ContentMask<Pixels>,
    ) -> (ContentMask<Pixels>, Option<(ScrollNodeId, Bounds<Pixels>)>) {
        match self.next_frame.scroll_node_stack.last() {
            Some(&top) => {
                let node = &self.next_frame.scroll_nodes[top as usize];
                let real = ContentMask { bounds: mask.bounds.intersect(&node.clip) };
                (real, Some((node.id, mask.bounds)))
            }
            None => (mask, None),
        }
    }

    /// Paint something that depends on `handle`'s offset outside its node
    /// (a scrollbar thumb): a scrolled replay of a record holding it while
    /// that node moved needs a build (`plan_scrolled_replay`).
    pub fn paint_scroll_dependent(&mut self, handle: &ScrollHandle) {
        let at = self.next_frame.scene.paint_operations.len();
        let space = self.next_frame.scene.scroll.space;
        self.next_frame.scroll_dependents.push((ScrollNodeId::of(handle), at, space));
    }

    /// `paint_scroll_dependent` for the innermost scroll node painting now
    /// (a sticky box: its place follows that scroller's offset).
    pub fn paint_dependent_on_open_scroll_node(&mut self) {
        let frame = &mut self.next_frame;
        if let Some(&(index, _)) = frame.scroll_paint.open.last() {
            let id = frame.scroll_nodes[index as usize].id;
            let at = frame.scene.paint_operations.len();
            frame.scroll_dependents.push((id, at, frame.scene.scroll.space));
        }
    }

    /// Whether a dependent painted in `scenes` of the rendered frame belongs
    /// (in scene `space`) to a node `moved` says moved.
    pub(crate) fn scroll_dependents_moved(&self, space: SceneSpace, scenes: &Range<usize>, moved: impl Fn(ScrollNodeId) -> bool) -> bool {
        self.rendered_frame
            .scroll_dependents
            .iter()
            .any(|&(id, at, of)| of == space && scenes.contains(&at) && moved(id))
    }
}