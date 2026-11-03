//! KaminIDE patch: the window's frame tree survives frames.
//!
//! A frame used to drop its Taffy tree, so every node lost its layout cache
//! (Taffy keys it by known dimensions, available space and parent size, the
//! same idea as the LayoutNG constraint-space cache). Now the frame tree keeps
//! the nodes of the last pass. A node requested again with the same inputs is
//! *claimed*: it keeps its Taffy node and with it the cached results of its
//! whole subtree, so a relayout recomputes only nodes whose inputs changed and
//! the ancestors of those nodes.
//!
//! The inputs of a node are its style, its children and, for a measured leaf,
//! its measure function. A function is opaque, so a measured leaf is claimed
//! only under a *layout key* its host supplies (`Window::set_next_layout_key`)
//! that covers everything the function reads. Identity:
//! - a parent is the previous parent of its first child, when it has exactly
//!   the same children in the same order and the same style;
//! - a plain leaf is the node at the same position of the previous pass's
//!   request sequence, when it is still a childless plain leaf of that style;
//! - a measured leaf is any unclaimed previous leaf with the same layout key
//!   and style (equal keys mean interchangeable measurements).
//!
//! Nothing is ever marked dirty: a node whose inputs differ is a new node, so
//! its ancestors are new nodes as well, and new nodes start with empty caches.
//! At the end of a pass (`TaffyLayoutEngine::clear`) the previous nodes that
//! were not claimed are dropped in one sweep (`TaffyTree::retain_nodes`).

use super::{LayoutId, NodeContext, TaffyLayoutEngine};
use collections::{FxHashMap, FxHashSet};
use taffy::tree::NodeId;

/// Request history of the frame tree across passes.
#[derive(Default)]
pub(super) struct Retained {
    /// Nodes in request order of the previous pass.
    prev: Vec<NodeId>,
    /// Nodes in request order of this pass.
    seq: Vec<NodeId>,
    /// Nodes requested (claimed or created) in this pass.
    live: FxHashSet<NodeId>,
    /// Layout keys of the measured leaves in the tree.
    keys: FxHashMap<NodeId, u64>,
    /// Unclaimed measured leaves of the previous pass by layout key.
    by_key: FxHashMap<u64, Vec<NodeId>>,
    /// Layout key for the next measured leaf request.
    pub(super) next_key: Option<u64>,
    /// Whether the host vouches for its layout keys now (Window::with_layout_keys).
    pub(super) keys_enabled: bool,
    /// Nodes claimed from a previous pass, for counters and tests.
    pub(super) claimed: u64,
}

impl Retained {
    fn note(&mut self, node: NodeId) {
        self.live.insert(node);
        self.seq.push(node);
    }
}

impl TaffyLayoutEngine {
    /// The previous node that a parent request with `style` and `children`
    /// claims, if any.
    pub(super) fn claim_parent(
        &mut self,
        style: &taffy::Style,
        children: &[LayoutId],
    ) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let first = children.first()?.0;
        let parent = self.taffy.parent(first)?;
        let found = !self.retained.live.contains(&parent)
            && !self.taffy.has_node_context(parent)
            && self.taffy.children_slice(parent) == LayoutId::to_taffy_slice(children)
            && children
                .iter()
                .all(|child| self.taffy.parent(child.0) == Some(parent))
            && self.taffy.style(parent).is_ok_and(|old| old == style);
        found.then(|| self.claim(parent))
    }

    /// The previous plain leaf that a leaf request with `style` claims.
    pub(super) fn claim_leaf(&mut self, style: &taffy::Style) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.seq.len())?;
        let found = !self.retained.live.contains(&node)
            && self.taffy.style(node).is_ok_and(|old| old == style)
            && self.taffy.children_slice(node).is_empty()
            && !self.taffy.has_node_context(node);
        found.then(|| self.claim(node))
    }

    /// The previous measured leaf that a keyed request claims. The new
    /// measure function replaces the old one; the leaf's measured outputs
    /// (static positions, line baselines) and cache stay.
    pub(super) fn claim_measured(
        &mut self,
        key: u64,
        style: &taffy::Style,
        context: &mut Option<NodeContext>,
    ) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let candidates = self.retained.by_key.get_mut(&key)?;
        let live = &self.retained.live;
        let at = candidates.iter().rposition(|node| {
            !live.contains(node)
                && self.taffy.style(*node).is_ok_and(|old| old == style)
                && self.taffy.children_slice(*node).is_empty()
        })?;
        let node = candidates.swap_remove(at);
        let fresh = context.take()?;
        if let Some(old) = self.taffy.get_node_context_mut(node) {
            old.measure = fresh.measure;
        }
        Some(self.claim(node))
    }

    /// An unclaimed previous node at this request's position, whose storage a
    /// new node takes (TaffyTree::reset_node): a pass that claims little
    /// then holds about one tree, not the previous and the new one.
    pub(super) fn recycle(&mut self) -> Option<NodeId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.seq.len())?;
        if self.retained.live.contains(&node) {
            return None;
        }
        self.retained.keys.remove(&node);
        Some(node)
    }

    fn claim(&mut self, node: NodeId) -> LayoutId {
        self.retained.claimed += 1;
        self.retained.note(node);
        LayoutId(node)
    }

    /// Register a node created in this pass; `key` is its layout key.
    pub(super) fn created(&mut self, node: LayoutId, key: Option<u64>) -> LayoutId {
        if self.frame_tree {
            self.retained.note(node.0);
            if let Some(key) = key {
                self.retained.keys.insert(node.0, key);
            }
        }
        node
    }

    /// End a pass of the frame tree: drop the nodes this pass did not request
    /// and make its requests the candidates of the next pass.
    pub(super) fn retire(&mut self) {
        let retained = &mut self.retained;
        let live = std::mem::take(&mut retained.live);
        self.taffy.retain_nodes(|node| live.contains(&node));
        retained.keys.retain(|node, _| live.contains(node));
        retained.by_key.clear();
        for (node, key) in &retained.keys {
            retained.by_key.entry(*key).or_default().push(*node);
        }
        retained.prev = std::mem::take(&mut retained.seq);
        retained.next_key = None;
        retained.keys_enabled = false;
        retained.live = live;
        retained.live.clear();
    }

    /// Forget every retained node (a discarded or panicking pass).
    pub(super) fn forget(&mut self) {
        self.retained = Retained::default();
    }

    /// Nodes claimed from previous passes since the engine was created.
    pub fn claimed_nodes(&self) -> u64 {
        self.retained.claimed
    }
}
