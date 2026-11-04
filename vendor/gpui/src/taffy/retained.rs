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

use super::{LayoutId, NodeContext, NodeMeasureFn, TaffyLayoutEngine, node_measure};
use collections::{FxHashMap, FxHashSet};
use std::any::Any;
use std::rc::Rc;
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
    /// Stamp of each node: issued when the node is created, kept while it is
    /// claimed. A claimed parent has only claimed children, so an equal stamp
    /// means the whole subtree has the inputs it had when it was stamped.
    stamps: FxHashMap<NodeId, u64>,
    last_stamp: u64,
    /// Host values derived from a node's subtree (`Window::set_layout_memo`),
    /// valid while the node keeps its stamp.
    memos: FxHashMap<NodeId, Rc<dyn Any>>,
    /// Requested styles of nodes a pass overrode in place (shrink-to-fit caps,
    /// root stretch): restored at the end of the pass, so the next request
    /// with the same style claims the node again.
    overrides: FxHashMap<NodeId, taffy::Style>,
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
        let live = &self.retained.live;
        let taffy = &self.taffy;
        let fits = |node: &NodeId| {
            !live.contains(node)
                && taffy.style(*node).is_ok_and(|old| old == style)
                && taffy.children_slice(*node).is_empty()
        };
        // The leaf at the same request position first: leaves of one key are
        // interchangeable, but keeping each at its place keeps its parent.
        let at_position = self
            .retained
            .prev
            .get(self.retained.seq.len())
            .copied()
            .filter(|node| self.retained.keys.get(node) == Some(&key) && fits(node));
        let node = match at_position {
            Some(node) => node,
            None => {
                let candidates = self.retained.by_key.get_mut(&key)?;
                // Leaves claimed at their positions stay listed; drop them lazily.
                while candidates.last().is_some_and(|node| live.contains(node)) {
                    candidates.pop();
                }
                let at = candidates.iter().rposition(fits)?;
                candidates.swap_remove(at)
            }
        };
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
            self.retained.last_stamp += 1;
            let stamp = self.retained.last_stamp;
            self.retained.stamps.insert(node.0, stamp);
            self.retained.memos.remove(&node.0);
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
        for (node, style) in retained.overrides.drain() {
            if live.contains(&node) {
                let _ = self.taffy.set_style(node, style);
            }
        }
        self.taffy.retain_nodes(|node| live.contains(&node));
        retained.stamps.retain(|node, _| live.contains(node));
        retained.memos.retain(|node, _| live.contains(node));
        // A kept leaf is measured again only by the function of the request
        // that claims it, so the old function and everything it captured (a
        // whole paragraph probe) need not outlive the frame.
        let stub = retired_measure();
        for node in &live {
            if self.taffy.has_node_context(*node)
                && let Some(context) = self.taffy.get_node_context_mut(*node)
            {
                context.measure = stub.clone();
            }
        }
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

    /// Remember the requested style of `node` before the pass overrides it.
    pub(super) fn keep_requested_style(&mut self, node: NodeId) {
        if self.frame_tree
            && !self.retained.overrides.contains_key(&node)
            && let Ok(style) = self.taffy.style(node)
        {
            self.retained.overrides.insert(node, style.clone());
        }
    }

    /// The stamp of a node of this pass (see `Retained::stamps`).
    pub fn layout_stamp(&self, id: LayoutId) -> Option<u64> {
        self.retained.stamps.get(&id.0).copied()
    }

    /// The host memo of a node, if the node kept its stamp since it was set.
    pub fn layout_memo(&self, id: LayoutId) -> Option<Rc<dyn Any>> {
        self.retained.memos.get(&id.0).cloned()
    }

    /// Attach a host value derived only from the node's subtree inputs; it
    /// lives while the node is claimed and is dropped with the node.
    pub fn set_layout_memo(&mut self, id: LayoutId, memo: Rc<dyn Any>) {
        if self.retained.stamps.contains_key(&id.0) {
            self.retained.memos.insert(id.0, memo);
        }
    }

    /// Nodes claimed from previous passes since the engine was created.
    pub fn claimed_nodes(&self) -> u64 {
        self.retained.claimed
    }
}

/// The measure function of a leaf between passes: a kept leaf gets the function
/// of the request that claims it before any layout, so this one never measures.
fn retired_measure() -> std::rc::Rc<std::cell::RefCell<NodeMeasureFn>> {
    std::rc::Rc::new(std::cell::RefCell::new(node_measure(Box::new(
        |_, _, _, _: &mut crate::Window, _: &mut crate::App| {
            let none = crate::size(crate::Pixels(0.), crate::Pixels(0.));
            crate::MeasuredContent::from((none, None, None, None))
        },
    ))))
}
