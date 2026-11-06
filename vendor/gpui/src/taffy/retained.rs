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
//!   and style (equal keys mean interchangeable measurements), the one at the
//!   same request position first.
//!
//! Styles are compared by value (shared allocation first, `style_share`); a
//! claimed node takes this pass's shared allocation of its equal style.
//!
//! Nothing is ever marked dirty: a node whose inputs differ is a new node, so
//! its ancestors are new nodes as well, and new nodes start with empty caches.
//! At the end of a pass (`TaffyLayoutEngine::clear`) the previous nodes that
//! were not claimed are dropped in one sweep (`TaffyTree::retain_nodes`).
//!
//! The bookkeeping lives on the nodes where it can: a key's candidate slot in
//! the measured leaf's context, the pinned identity of a stamped node in its
//! Taffy node; only keys and memos are side tables.

use super::{LayoutId, NodeContext, NodeMeasureFn, TaffyLayoutEngine, node_measure};
use collections::{FxHashMap, FxHashSet};
use std::any::Any;
use std::rc::Rc;
use taffy::tree::NodeId;

/// `NodeContext::key_slot` of a leaf that is in no candidate list.
pub(super) const NO_SLOT: u32 = u32::MAX;

/// Request history of the frame tree across passes.
#[derive(Default)]
pub(super) struct Retained {
    /// Nodes in request order of the previous pass.
    prev: Vec<NodeId>,
    /// Nodes in request order of this pass.
    seq: Vec<NodeId>,
    /// Nodes requested (claimed or created) in this pass.
    live: FxHashSet<NodeId>,
    /// Layout keys of the keyed measured leaves in the tree.
    keys: FxHashMap<NodeId, u64>,
    /// Unclaimed measured leaves of the previous pass by layout key. Each
    /// leaf's index in its list is `NodeContext::key_slot`, so a claimed or
    /// recycled leaf leaves its list in O(1) and a key shared by many leaves
    /// never makes a request scan the leaves already claimed.
    pub(super) by_key: FxHashMap<u64, Vec<NodeId>>,
    /// Layout key for the next measured leaf request.
    pub(super) next_key: Option<u64>,
    /// Whether the host vouches for its layout keys now (Window::with_layout_keys).
    pub(super) keys_enabled: bool,
    /// Nodes claimed from a previous pass, for counters and tests.
    pub(super) claimed: u64,
    /// Host values derived from a node's subtree (`Window::set_layout_memo`),
    /// valid while the node keeps its stamp.
    memos: FxHashMap<NodeId, Rc<dyn Any>>,
    /// Requested styles of nodes a pass overrode in place (shrink-to-fit caps,
    /// root stretch): restored at the end of the pass, so the next request
    /// with the same style claims the node again.
    overrides: FxHashMap<NodeId, taffy::SharedStyle>,
    /// A measure function of a kept leaf ran in this pass. Only a leaf of a
    /// subtree claimed whole (`claim_subtree`) keeps the retired function, so
    /// its constraints changed and the caches of this pass's ancestors hold
    /// results computed from a stub: the host rebuilds the frame and the tree
    /// is dropped at the end of the pass.
    stale: Rc<std::cell::Cell<bool>>,
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
        style: &taffy::SharedStyle,
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
            && self.taffy.style_equals(parent, style);
        found.then(|| self.claim(parent, style))
    }

    /// The previous plain leaf that a leaf request with `style` claims.
    pub(super) fn claim_leaf(&mut self, style: &taffy::SharedStyle) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.seq.len())?;
        let found = !self.retained.live.contains(&node)
            && self.taffy.style_equals(node, style)
            && self.taffy.children_slice(node).is_empty()
            && !self.taffy.has_node_context(node);
        found.then(|| self.claim(node, style))
    }

    /// The previous measured leaf that a keyed request claims. The new
    /// measure function replaces the old one; the leaf's measured outputs
    /// (static positions, line baselines) and cache stay.
    pub(super) fn claim_measured(
        &mut self,
        key: u64,
        style: &taffy::SharedStyle,
        context: &mut Option<NodeContext>,
    ) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let live = &self.retained.live;
        let taffy = &self.taffy;
        let fits = |node: &NodeId| {
            !live.contains(node)
                && taffy.style_equals(*node, style)
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
                let candidates = self.retained.by_key.get(&key)?;
                candidates[candidates.iter().rposition(fits)?]
            }
        };
        self.unlist(key, node);
        let fresh = context.take()?;
        if let Some(old) = self.taffy.get_node_context_mut(node) {
            old.measure = fresh.measure;
        }
        Some(self.claim(node, style))
    }

    /// Take the previous measured leaf `node` out of the candidates of `key`.
    fn unlist(&mut self, key: u64, node: NodeId) {
        let Some(context) = self.taffy.get_node_context_mut(node) else {
            return;
        };
        let at = std::mem::replace(&mut context.key_slot, NO_SLOT);
        let Some(list) = self.retained.by_key.get_mut(&key) else {
            return;
        };
        let at = at as usize;
        if list.get(at) == Some(&node) {
            list.swap_remove(at);
            if let Some(&moved) = list.get(at)
                && let Some(context) = self.taffy.get_node_context_mut(moved)
            {
                context.key_slot = at as u32;
            }
        }
    }

    /// An unclaimed previous node at this request's position, whose storage a
    /// new node takes (TaffyTree::reset_node): a pass that claims little
    /// then holds about one tree, not the previous and the new one. A node
    /// with a pinned identity (`layout_stamp`) is never reused: its id must
    /// not name another subtree.
    pub(super) fn recycle(&mut self) -> Option<NodeId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.seq.len())?;
        if self.retained.live.contains(&node) || self.taffy.identity_pinned(node) {
            return None;
        }
        if let Some(key) = self.retained.keys.remove(&node) {
            self.unlist(key, node);
        }
        Some(node)
    }

    /// Claim `node`, whose style equals `style`. The node takes the shared allocation
    /// of this pass (`style_share`): the value is the same, so its cache stays valid,
    /// and the allocation of the previous pass is freed with its last node.
    fn claim(&mut self, node: NodeId, style: &taffy::SharedStyle) -> LayoutId {
        let adopted = self.taffy.adopt_equal_style(node, style);
        debug_assert!(adopted, "a claimed node has the requested style");
        self.retained.claimed += 1;
        self.retained.note(node);
        LayoutId(node)
    }

    /// Register a node created in this pass; `key` is its layout key.
    pub(super) fn created(&mut self, node: LayoutId, key: Option<u64>) -> LayoutId {
        if self.frame_tree {
            self.retained.note(node.0);
            debug_assert!(
                !self.retained.memos.contains_key(&node.0),
                "a reused node never had a stamp, so it has no memo"
            );
            if let Some(key) = key {
                self.retained.keys.insert(node.0, key);
            }
        }
        node
    }

    /// End a pass of the frame tree: drop the nodes this pass did not request
    /// and make its requests the candidates of the next pass.
    pub(super) fn retire(&mut self) {
        if self.retained.stale.get() {
            self.forget();
            self.taffy.clear();
            return;
        }
        let retained = &mut self.retained;
        let live = std::mem::take(&mut retained.live);
        // Taken, not drained: the table is freed between passes.
        for (node, style) in std::mem::take(&mut retained.overrides) {
            if live.contains(&node) {
                let _ = self.taffy.set_style(node, style);
            }
        }
        self.taffy.retain_nodes(|node| live.contains(&node));
        retained.memos.retain(|node, _| live.contains(node));
        // A kept leaf is measured again only by the function of the request
        // that claims it, so the old function and everything it captured (a
        // whole paragraph probe) need not outlive the frame.
        let stub = retired_measure(retained.stale.clone());
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
            let list = retained.by_key.entry(*key).or_default();
            if let Some(context) = self.taffy.get_node_context_mut(*node) {
                context.key_slot = list.len() as u32;
            }
            list.push(*node);
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
            && let Ok(style) = self.taffy.shared_style(node)
        {
            self.retained.overrides.insert(node, style);
        }
    }

    /// The stamp of a node of this pass: its id, pinned so that no other
    /// subtree takes it over (`recycle`). A node keeps its id while passes
    /// claim it, and a claimed parent has only claimed children, so an equal
    /// stamp means the whole subtree has the inputs it had when it was stamped.
    pub fn layout_stamp(&mut self, id: LayoutId) -> Option<u64> {
        if !self.frame_tree || !self.retained.live.contains(&id.0) {
            return None;
        }
        self.taffy.pin_identity(id.0);
        Some(u64::from(id.0))
    }

    /// The host memo of a node, if the node kept its stamp since it was set.
    pub fn layout_memo(&self, id: LayoutId) -> Option<Rc<dyn Any>> {
        self.retained.memos.get(&id.0).cloned()
    }

    /// Attach a host value derived only from the node's subtree inputs; it
    /// lives while the node is claimed and is dropped with the node.
    pub fn set_layout_memo(&mut self, id: LayoutId, memo: Rc<dyn Any>) {
        if self.layout_stamp(id).is_some() {
            self.retained.memos.insert(id.0, memo);
        }
    }

    /// Request position of this pass: a subtree requested from here on is
    /// captured by `capture_subtree`.
    pub fn request_position(&self) -> usize {
        self.retained.seq.len()
    }

    /// The nodes requested since `start` when they form the subtree of `root`
    /// (requested last), with the stamp of `root`. A later pass that keeps the
    /// stamp may claim the subtree whole (`claim_subtree`).
    pub fn capture_subtree(&mut self, start: usize, root: LayoutId) -> Option<(u64, Rc<[NodeId]>)> {
        let nodes = self.retained.seq.get(start..)?;
        if nodes.last() != Some(&root.0) {
            return None;
        }
        let nodes: Rc<[NodeId]> = nodes.into();
        let stamp = self.layout_stamp(root)?;
        Some((stamp, nodes))
    }

    /// Claim a subtree captured in a previous pass without its requests. The
    /// stamp of the root proves that every node of the subtree was kept with
    /// its inputs; the leaves keep the retired measure function, so a layout
    /// that needs a new measurement marks the pass stale (`layout_stale`).
    /// The root's id is its stamp: a pinned node is never recycled and a swept
    /// one leaves its slot with a new version, so a live pinned root with that
    /// id is the stamped node.
    pub fn claim_subtree(&mut self, stamp: u64, nodes: &[NodeId]) -> Option<LayoutId> {
        let root = *nodes.last()?;
        if !self.frame_tree
            || u64::from(root) != stamp
            || !self.taffy.identity_pinned(root)
            || nodes.iter().any(|node| self.retained.live.contains(node))
        {
            return None;
        }
        for &node in nodes {
            // A keyed leaf leaves its candidate list, as when its request claims it.
            if self
                .taffy
                .get_node_context(node)
                .is_some_and(|context| context.key_slot != NO_SLOT)
                && let Some(&key) = self.retained.keys.get(&node)
            {
                self.unlist(key, node);
            }
            self.retained.note(node);
        }
        self.retained.claimed += nodes.len() as u64;
        Some(LayoutId(root))
    }

    /// Whether a leaf of a subtree claimed whole was measured in this pass.
    pub fn layout_stale(&self) -> bool {
        self.retained.stale.get()
    }

    /// Nodes claimed from previous passes since the engine was created.
    pub fn claimed_nodes(&self) -> u64 {
        self.retained.claimed
    }
}

/// The measure function of a leaf between passes: a leaf claimed by its request
/// gets that request's function before any layout, so this one measures only a
/// leaf of a subtree claimed whole whose constraints changed; it marks the pass.
fn retired_measure(
    stale: Rc<std::cell::Cell<bool>>,
) -> std::rc::Rc<std::cell::RefCell<NodeMeasureFn>> {
    std::rc::Rc::new(std::cell::RefCell::new(node_measure(Box::new(
        move |_, _, _, _: &mut crate::Window, _: &mut crate::App| {
            stale.set(true);
            let none = crate::size(crate::Pixels(0.), crate::Pixels(0.));
            crate::MeasuredContent::from((none, None, None, None))
        },
    ))))
}
