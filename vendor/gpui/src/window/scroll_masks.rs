//! KaminIDE patch: masks under scroll nodes (`scroll_nodes`).
//!
//! The children of a scroll node are prepainted and painted under the node's
//! interest rect instead of its clip, so their own masks (`scene::scroll_marks`)
//! cover what a replay at another offset may show. Each primitive and hitbox
//! is drawn and hit-tested with own mask ∩ the node's real clip chain.

mod viewport;

use super::*;
use crate::ScrollHandle;
use crate::scene::{Primitive, Scene, scroll_marks::SceneSpace};

/// The scroll node and own mask of a hitbox inserted under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HitboxMark {
    /// Index of the hitbox in the frame's `hitboxes`.
    pub(crate) index: u32,
    pub(crate) node: ScrollNodeId,
    pub(crate) own: Bounds<Pixels>,
}

impl Frame {
    /// Run `f` on the scene with the innermost open paint node as the node
    /// its commands land under (`Scene::mark_scrolled`, `mark_layer`,
    /// `mark_copied`): its id and real clip chain in scene pixels. A
    /// primitive the scene could not keep taints the node.
    pub(crate) fn with_scene_node<R>(&mut self, scale: f32, f: impl FnOnce(&mut Scene) -> R) -> R {
        let open = self.scroll_paint.open.last().map(|&(index, _)| index);
        self.scene.scroll.open = open.map(|index| {
            let node = &self.scroll_nodes[index as usize];
            (node.id, node.clip.scale(scale))
        });
        let result = f(&mut self.scene);
        self.scene.scroll.open = None;
        if std::mem::take(&mut self.scene.scroll.lost) {
            if let Some(index) = open {
                self.scroll_nodes[index as usize].torn = true;
            }
        }
        result
    }
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
        self.next_frame.with_scene_node(scale, |scene| scene.insert_primitive(primitive));
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
    /// other than by a translation (a follower node moves those): a
    /// scrolled replay of a record holding it while that node moved needs a
    /// build (`plan_scrolled_replay`).
    pub fn paint_scroll_dependent(&mut self, handle: &ScrollHandle) {
        let at = self.next_frame.scene.paint_operations.len();
        let space = self.next_frame.scene.scroll.space;
        self.next_frame.scroll_dependents.push((ScrollNodeId::of(handle), at, space));
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