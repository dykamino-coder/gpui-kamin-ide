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
//! Positions are aligned, not absolute: a claimed measured leaf moves the
//! cursor into the previous sequence to the position after its own, so a
//! subtree of another size before it (a block replaced by one leaf, a
//! paragraph that gained a node) shifts only the requests up to the next
//! keyed leaf. A position is only a guess at identity: whatever node it
//! finds is claimed only when it is a childless plain leaf of an equal style,
//! whose layout depends on nothing else.
//!
//! Equal keys repeat (equal texts), so a keyed leaf moves the cursor only
//! while the cursor may be off (`astray`: a node was created since the last
//! anchor, or a parent was claimed away from the cursor). While the cursor
//! is aligned, a keyed request takes the leaf at the cursor or one left
//! behind it, never one ahead: that one belongs to a later request (a cell
//! whose text became that of a later cell), and taking it would move the
//! cursor there and set every later request against the wrong nodes.
//!
//! Styles are compared by value (shared allocation first, `style_share`); a
//! claimed node takes this pass's shared allocation of its equal style.
//!
//! Nothing is ever marked dirty: a node whose inputs differ is a new node, so
//! its ancestors are new nodes as well, and new nodes start with empty caches.
//! At the end of a pass (`TaffyLayoutEngine::clear`) the previous nodes that
//! were not claimed are dropped in one sweep (`TaffyTree::retain_nodes`).
//!
//! A host may keep the measure functions of a subtree's leaves for the next
//! pass (`keep_measures`): that pass can claim the subtree whole and lay it
//! out at new constraints (another available width) without its requests.
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

/// Mark of a position whose leaf left its candidate list (`Candidates`).
const GONE: u32 = 1 << 31;

/// Previous-pass positions of the measured leaves of one layout key,
/// ascending, so the one nearest the cursor is found by bisection. A leaf
/// that is claimed (by its request or with a whole subtree) or recycled leaves
/// its list in O(1): its entry, found through `NodeContext::key_slot`, keeps
/// its position for the bisection and gets the `GONE` mark, and `head` skips
/// the marked entries at the front. Requests mostly claim in order, so a key
/// shared by many leaves does not make a request scan the claimed ones.
#[derive(Default)]
pub(super) struct Candidates {
    pub(super) positions: Vec<u32>,
    head: usize,
}

#[cfg(test)]
impl Candidates {
    /// No unclaimed leaf is left.
    pub(super) fn is_empty(&self) -> bool {
        self.positions[self.head..].iter().all(|p| p & GONE != 0)
    }
}

/// Request history of the frame tree across passes.
#[derive(Default)]
pub(super) struct Retained {
    /// Nodes in request order of the previous pass.
    prev: Vec<NodeId>,
    /// Nodes in request order of this pass.
    seq: Vec<NodeId>,
    /// Position in `prev` that corresponds to the next request of this pass.
    cursor: usize,
    /// The cursor may be off: a node was created since it was last confirmed
    /// (a keyed leaf claimed at the cursor, a parent claimed at the cursor).
    astray: bool,
    /// Nodes requested (claimed or created) in this pass.
    live: FxHashSet<NodeId>,
    /// Layout keys of the keyed measured leaves in the tree.
    keys: FxHashMap<NodeId, u64>,
    /// Positions in `prev` of the previous pass's measured leaves by layout
    /// key (`Candidates`).
    pub(super) by_key: FxHashMap<u64, Candidates>,
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
    /// Leaves of this pass whose measure functions the next pass keeps
    /// (`keep_measures`); emptied at the end of every pass, capacity kept.
    keep: FxHashSet<NodeId>,
}

impl Retained {
    fn note(&mut self, node: NodeId) {
        self.live.insert(node);
        self.seq.push(node);
        self.cursor += 1;
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
        if found {
            // A parent is known by its children: claimed at the cursor, it
            // confirms the alignment; elsewhere, it shows the cursor is off.
            self.retained.astray = self.retained.prev.get(self.retained.cursor) != Some(&parent);
        }
        found.then(|| self.claim(parent, style))
    }

    /// The previous plain leaf that a leaf request with `style` claims.
    pub(super) fn claim_leaf(&mut self, style: &taffy::SharedStyle) -> Option<LayoutId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.cursor)?;
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
        let cursor = self.retained.cursor;
        let candidates = self.retained.by_key.get(&key)?;
        let (prev, live) = (&self.retained.prev, &self.retained.live);
        let fits = |position: &u32| {
            let node = prev[(*position & !GONE) as usize];
            *position & GONE == 0
                && !live.contains(&node)
                && self.taffy.style_equals(node, style)
                && self.taffy.children_slice(node).is_empty()
        };
        // Equal keys are interchangeable, but the one nearest the cursor keeps
        // the requests after it aligned (equal texts repeat in a document).
        let positions = &candidates.positions;
        let head = candidates.head;
        let start =
            head + positions[head..].partition_point(|&position| (position & !GONE) < cursor as u32);
        let after = (start..positions.len()).find(|&at| fits(&positions[at]));
        let before = (head..start).rev().find(|&at| fits(&positions[at]));
        let distance = |at: usize| ((positions[at] & !GONE) as usize).abs_diff(cursor);
        let astray = self.retained.astray;
        let at = match (before, after) {
            (_, Some(a)) if distance(a) == 0 => a,
            // An aligned cursor: a leaf ahead is a later request's.
            (b, _) if !astray => b?,
            (Some(b), Some(a)) if distance(b) < distance(a) => b,
            (_, Some(a)) => a,
            (b, None) => b?,
        };
        let position = positions[at] as usize;
        let node = prev[position];
        self.unlist(key, node);
        let fresh = context.take()?;
        if let Some(old) = self.taffy.get_node_context_mut(node) {
            old.measure = fresh.measure;
        }
        // The requests after this leaf follow the ones after it last pass; a
        // leaf left behind an aligned cursor is new content, the cursor stays.
        if astray || position == cursor {
            self.retained.cursor = position;
            self.retained.astray = false;
        }
        Some(self.claim(node, style))
    }

    /// Take the previous measured leaf `node` out of the candidates of `key`.
    fn unlist(&mut self, key: u64, node: NodeId) {
        let Some(context) = self.taffy.get_node_context_mut(node) else {
            return;
        };
        let at = std::mem::replace(&mut context.key_slot, NO_SLOT) as usize;
        let prev = &self.retained.prev;
        let Some(list) = self.retained.by_key.get_mut(&key) else {
            return;
        };
        if let Some(position) = list.positions.get_mut(at)
            && prev.get((*position & !GONE) as usize) == Some(&node)
        {
            *position |= GONE;
        }
        while list.positions.get(list.head).is_some_and(|p| p & GONE != 0) {
            list.head += 1;
        }
    }

    /// An unclaimed previous node at this request's position, whose storage a
    /// new node takes (TaffyTree::reset_node): a pass that claims little
    /// then holds about one tree, not the previous and the new one. A keyed
    /// leaf is taken only by a `measured` request: until the cursor is aligned
    /// again the position may belong to a later request that would claim the
    /// leaf by its key, and taking it would renew that request's ancestors too.
    /// A node with a pinned identity (`layout_stamp`) is never reused: its id
    /// must not name another subtree.
    pub(super) fn recycle(&mut self, measured: bool) -> Option<NodeId> {
        if !self.frame_tree {
            return None;
        }
        let node = *self.retained.prev.get(self.retained.cursor)?;
        if self.retained.live.contains(&node) || self.taffy.identity_pinned(node) {
            return None;
        }
        if !measured && self.retained.keys.contains_key(&node) {
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
            self.retained.astray = true;
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
        // whole paragraph probe) need not outlive the frame. Leaves the host
        // keeps (`keep_measures`) keep their functions for one more pass.
        let stub = retired_measure(retained.stale.clone());
        let mut keep = std::mem::take(&mut retained.keep);
        for node in &live {
            if !keep.contains(node)
                && self.taffy.has_node_context(*node)
                && let Some(context) = self.taffy.get_node_context_mut(*node)
            {
                context.measure = stub.clone();
            }
        }
        keep.clear();
        retained.keep = keep;
        retained.keys.retain(|node, _| live.contains(node));
        retained.by_key.clear();
        for (position, node) in retained.seq.iter().enumerate() {
            if let Some(key) = retained.keys.get(node) {
                let list = &mut retained.by_key.entry(*key).or_default().positions;
                if let Some(context) = self.taffy.get_node_context_mut(*node) {
                    context.key_slot = list.len() as u32;
                }
                list.push(position as u32);
            }
        }
        retained.prev = std::mem::take(&mut retained.seq);
        retained.cursor = 0;
        retained.astray = false;
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

    /// Keep the measure functions of the measured leaves among `nodes`
    /// (requested in this pass) for the next pass, instead of retiring them:
    /// a subtree the next pass claims whole (`claim_subtree`) is then laid
    /// out at any new constraints by the functions of its own requests, like
    /// a fresh request of the same subtree, and the pass does not go stale.
    /// The host vouches that the functions read nothing that changes between
    /// the passes; it keeps them pass by pass, as long as it claims them.
    pub fn keep_measures(&mut self, nodes: &[NodeId]) {
        if !self.frame_tree {
            return;
        }
        for &node in nodes {
            if self.retained.live.contains(&node) && self.taffy.has_node_context(node) {
                self.retained.keep.insert(node);
            }
        }
    }

    /// Union of the absolute layout bounds of `nodes` in this pass.
    pub fn nodes_layout_bounds(
        &mut self,
        nodes: &[NodeId],
        scale_factor: f32,
    ) -> Option<crate::Bounds<crate::Pixels>> {
        nodes
            .iter()
            .map(|&node| self.layout_bounds(LayoutId(node), scale_factor))
            .reduce(|a, b| a.union(&b))
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
