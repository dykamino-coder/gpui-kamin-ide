use crate::{Bounds, Half};
use std::{
    cmp,
    fmt::Debug,
    ops::{Add, Sub},
    ptr::NonNull,
};

/// Maximum children per internal node (R-tree style branching factor).
/// Higher values = shorter tree = fewer cache misses, but more work per node.
const MAX_CHILDREN: usize = 12;

/// KaminIDE patch: minimum children of a node made by a split (about 40% of
/// `MAX_CHILDREN`, as in the R*-tree).
const MIN_CHILDREN: usize = 4;

/// A spatial tree optimized for finding maximum ordering among intersecting bounds.
///
/// This is an R-tree variant specifically designed for the use case of assigning
/// z-order to overlapping UI elements. Key optimizations:
/// - Tracks the leaf with global max ordering for O(1) fast-path queries
/// - Uses higher branching factor (`MAX_CHILDREN`) for lower tree height
/// - KaminIDE patch: stays balanced, splitting full nodes (`insert_leaf`)
/// - Aggressive pruning during search based on max_order metadata
#[derive(Debug)]
pub(crate) struct BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// All nodes stored contiguously for cache efficiency.
    nodes: Vec<Node<U>>,
    /// Index of the root node, if any.
    root: Option<usize>,
    /// Index of the leaf with the highest ordering (for fast-path lookups).
    max_leaf: Option<usize>,
    /// Reusable stack for tree traversal during insertion.
    insert_path: Vec<usize>,
    /// Reusable stack for search operations.
    search_stack: Vec<NonNull<Node<U>>>,
    /// KaminIDE patch: internal levels above the leaves (all leaves share one depth).
    height: usize,
    /// KaminIDE patch: reusable suffix covers of a node split.
    split_covers: Vec<Bounds<U>>,
}

/// A node in the bounds tree.
#[derive(Debug, Clone)]
struct Node<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// Bounding box containing this node and all descendants.
    bounds: Bounds<U>,
    /// Maximum ordering value in this subtree.
    max_order: u32,
    /// Node-specific data.
    kind: NodeKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    /// Leaf node containing actual bounds data.
    Leaf {
        /// The ordering assigned to this bounds.
        order: u32,
    },
    /// Internal node with children.
    Internal {
        /// Indices of child nodes (2 to MAX_CHILDREN).
        children: NodeChildren,
    },
}

/// Fixed-size array for child indices, avoiding heap allocation.
#[derive(Debug, Clone)]
struct NodeChildren {
    // Keeps an invariant where the max order child is always at the end
    indices: [usize; MAX_CHILDREN],
    len: u8,
}

impl NodeChildren {
    fn new() -> Self {
        Self {
            indices: [0; MAX_CHILDREN],
            len: 0,
        }
    }

    fn push(&mut self, index: usize) {
        debug_assert!((self.len as usize) < MAX_CHILDREN);
        self.indices[self.len as usize] = index;
        self.len += 1;
    }

    fn len(&self) -> usize {
        self.len as usize
    }

    fn as_slice(&self) -> &[usize] {
        &self.indices[..self.len as usize]
    }
}

impl<U> BoundsTree<U>
where
    U: Clone
        + Debug
        + PartialEq
        + PartialOrd
        + Add<U, Output = U>
        + Sub<Output = U>
        + Half
        + Default,
{
    /// Clears all nodes from the tree.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.max_leaf = None;
        self.insert_path.clear();
        self.search_stack.clear();
        self.height = 0;
        self.split_covers.clear();
    }

    /// Inserts bounds into the tree and returns its assigned ordering.
    ///
    /// The ordering is one greater than the maximum ordering of any
    /// existing bounds that intersect with the new bounds.
    pub fn insert(&mut self, new_bounds: Bounds<U>) -> u32 {
        self.insert_at_least(new_bounds, 1)
    }

    /// KaminIDE patch: вставка с порядком не ниже `min_order` — после
    /// переписанного порядка объёмного контекста (`Scene::pop_depth_context`).
    pub fn insert_at_least(&mut self, new_bounds: Bounds<U>, min_order: u32) -> u32 {
        // Find maximum ordering among intersecting bounds
        let max_intersecting = self.find_max_ordering(&new_bounds);
        let ordering = (max_intersecting + 1).max(min_order);

        // Insert the new leaf
        let new_leaf_idx = self.insert_leaf(new_bounds, ordering);

        // Update max_leaf tracking
        self.max_leaf = match self.max_leaf {
            None => Some(new_leaf_idx),
            Some(old_idx) if self.nodes[old_idx].max_order < ordering => Some(new_leaf_idx),
            some => some,
        };

        ordering
    }

    /// Finds the maximum ordering among all bounds that intersect with the query.
    fn find_max_ordering(&mut self, query: &Bounds<U>) -> u32 {
        let Some(root_idx) = self.root else {
            return 0;
        };

        // Fast path: check if the max-ordering leaf intersects
        if let Some(max_idx) = self.max_leaf {
            let max_node = &self.nodes[max_idx];
            if query.intersects(&max_node.bounds) {
                return max_node.max_order;
            }
        }

        // Slow path: search the tree
        self.search_stack.clear();
        self.search_stack.push(NonNull::from(&self.nodes[root_idx]));

        let mut max_found = 0u32;

        while let Some(node) = self.search_stack.pop() {
            // SAFETY: `node` is guaranteed to be valid as the `nodes` stack is unmodified in this function
            // and the `search_stack` only contains pointers from this function call.
            let node = unsafe { node.as_ref() };

            // Pruning: skip if this subtree can't improve our result
            if node.max_order <= max_found {
                continue;
            }

            // Spatial pruning: skip if bounds don't intersect
            if !query.intersects(&node.bounds) {
                continue;
            }

            match &node.kind {
                NodeKind::Leaf { order } => {
                    max_found = cmp::max(max_found, *order);
                }
                NodeKind::Internal { children } => {
                    // Children are maintained with highest max_order at the end.
                    // Push in forward order to highest (last) is popped first.
                    self.search_stack.extend(
                        children
                            .as_slice()
                            .iter()
                            .map(|&child_idx| &self.nodes[child_idx])
                            .filter(|node| node.max_order > max_found)
                            .map(NonNull::from),
                    );
                }
            }
        }

        max_found
    }

    /// Inserts a leaf node with the given bounds and ordering.
    /// Returns the index of the new leaf.
    ///
    /// KaminIDE patch: a balanced R-tree insert. Every leaf sits at the same
    /// depth under internal nodes. The descent picks the child that needs the
    /// least enlargement (Guttman, "R-trees", SIGMOD 1984, ChooseLeaf). A node
    /// that overflows splits along the axis and position with the smallest
    /// summed half-perimeter (the R*-tree split, Beckmann et al. 1990), and the
    /// split propagates up to the root. The previous descent never split a
    /// node: it nested a new pair under a full parent, so document-order text
    /// built chains about 20 levels deep, and every insert scanned hundreds of
    /// children. Queries are exact, so the assigned orderings do not change.
    fn insert_leaf(&mut self, bounds: Bounds<U>, order: u32) -> usize {
        let leaf = self.push_node(bounds.clone(), order, NodeKind::Leaf { order });
        let Some(root) = self.root else {
            let mut children = NodeChildren::new();
            children.push(leaf);
            self.root = Some(self.push_node(bounds, order, NodeKind::Internal { children }));
            self.height = 1;
            return leaf;
        };

        // Descend to the internal node whose children are leaves.
        self.insert_path.clear();
        let mut current = root;
        for _ in 1..self.height {
            self.insert_path.push(current);
            current = self.choose_child(current, &bounds);
        }
        self.insert_path.push(current);

        // Add bottom-up: a full node splits and hands its new sibling up.
        let mut entry = leaf;
        let mut level = self.insert_path.len();
        while level > 0 {
            level -= 1;
            let node = self.insert_path[level];
            let Some(sibling) = self.add_child(node, entry) else {
                // Every remaining ancestor only grows by the new leaf.
                let mut child = node;
                for &ancestor in self.insert_path[..level].iter().rev() {
                    self.grow(ancestor, child, &bounds, order);
                    child = ancestor;
                }
                return leaf;
            };
            entry = sibling;
            // The split moved children out of this node's subtree: its parent
            // keeps the same cover, but the max-order child may have moved.
            if level > 0 {
                let parent = self.insert_path[level - 1];
                self.grow(parent, node, &bounds, order);
                self.last_is_max(parent);
            }
        }

        // The root split: a new root holds both halves.
        let mut children = NodeChildren::new();
        children.push(root);
        children.push(entry);
        let root_bounds = self.nodes[root].bounds.union(&self.nodes[entry].bounds);
        let root_max = cmp::max(self.nodes[root].max_order, self.nodes[entry].max_order);
        let new_root = self.push_node(root_bounds, root_max, NodeKind::Internal { children });
        self.last_is_max(new_root);
        self.root = Some(new_root);
        self.height += 1;
        leaf
    }

    fn push_node(&mut self, bounds: Bounds<U>, max_order: u32, kind: NodeKind) -> usize {
        self.nodes.push(Node {
            bounds,
            max_order,
            kind,
        });
        self.nodes.len() - 1
    }

    fn children(&self, node: usize) -> &NodeChildren {
        match &self.nodes[node].kind {
            NodeKind::Internal { children } => children,
            NodeKind::Leaf { .. } => unreachable!("leaves have no children"),
        }
    }

    fn children_mut(&mut self, node: usize) -> &mut NodeChildren {
        match &mut self.nodes[node].kind {
            NodeKind::Internal { children } => children,
            NodeKind::Leaf { .. } => unreachable!("leaves have no children"),
        }
    }

    /// The child of `node` that grows least to cover `bounds` (ties: the smaller).
    fn choose_child(&self, node: usize, bounds: &Bounds<U>) -> usize {
        let children = self.children(node).as_slice();
        let mut best = children[0];
        let mut best_cost = None::<(U, U)>;
        for &child in children {
            let own = self.nodes[child].bounds.half_perimeter();
            let grown = bounds.union(&self.nodes[child].bounds).half_perimeter();
            let cost = (grown - own.clone(), own);
            let better = match &best_cost {
                None => true,
                Some((enlargement, size)) => {
                    cost.0 < *enlargement || (cost.0 == *enlargement && cost.1 < *size)
                }
            };
            if better {
                best = child;
                best_cost = Some(cost);
            }
        }
        best
    }

    /// Grows `node` by a leaf added under its child `child`; the child whose
    /// subtree now holds the highest ordering moves to the end.
    fn grow(&mut self, node: usize, child: usize, bounds: &Bounds<U>, order: u32) {
        let raised = self.nodes[node].max_order < order;
        let node_ref = &mut self.nodes[node];
        node_ref.bounds = node_ref.bounds.union(bounds);
        node_ref.max_order = cmp::max(node_ref.max_order, order);
        if raised {
            let children = self.children_mut(node);
            if let Some(pos) = children.as_slice().iter().position(|&c| c == child) {
                let last = children.len() - 1;
                children.indices.swap(pos, last);
            }
        }
    }

    /// Restores the invariant that the last child has the highest max ordering.
    fn last_is_max(&mut self, node: usize) {
        let children = self.children(node);
        let (mut best, mut best_order) = (0, 0);
        for (pos, &child) in children.as_slice().iter().enumerate() {
            if self.nodes[child].max_order >= best_order {
                (best, best_order) = (pos, self.nodes[child].max_order);
            }
        }
        let children = self.children_mut(node);
        let last = children.len() - 1;
        children.indices.swap(best, last);
    }

    /// Adds `child` under `node`, or splits a full `node` and returns the new
    /// sibling that holds part of its children.
    fn add_child(&mut self, node: usize, child: usize) -> Option<usize> {
        let child_bounds = self.nodes[child].bounds.clone();
        let child_order = self.nodes[child].max_order;
        if self.children(node).len() < MAX_CHILDREN {
            let last_order = (self.children(node).as_slice().last())
                .map_or(0, |&last| self.nodes[last].max_order);
            let children = self.children_mut(node);
            children.push(child);
            if child_order < last_order {
                let last = children.len() - 1;
                children.indices.swap(last - 1, last);
            }
            let node_ref = &mut self.nodes[node];
            node_ref.bounds = node_ref.bounds.union(&child_bounds);
            node_ref.max_order = cmp::max(node_ref.max_order, child_order);
            return None;
        }
        let mut entries = [0usize; MAX_CHILDREN + 1];
        entries[..MAX_CHILDREN].copy_from_slice(self.children(node).as_slice());
        entries[MAX_CHILDREN] = child;
        let (left, right) = self.split(&mut entries);
        let (left_bounds, left_max) = self.cover(left);
        let (right_bounds, right_max) = self.cover(right);
        let mut left_children = NodeChildren::new();
        left.iter().for_each(|&c| left_children.push(c));
        let mut right_children = NodeChildren::new();
        right.iter().for_each(|&c| right_children.push(c));
        self.nodes[node] = Node {
            bounds: left_bounds,
            max_order: left_max,
            kind: NodeKind::Internal {
                children: left_children,
            },
        };
        self.last_is_max(node);
        let sibling = self.push_node(
            right_bounds,
            right_max,
            NodeKind::Internal {
                children: right_children,
            },
        );
        self.last_is_max(sibling);
        Some(sibling)
    }

    /// The cover and highest ordering of `entries`.
    fn cover(&self, entries: &[usize]) -> (Bounds<U>, u32) {
        let mut bounds = self.nodes[entries[0]].bounds.clone();
        let mut max_order = self.nodes[entries[0]].max_order;
        for &entry in &entries[1..] {
            bounds = bounds.union(&self.nodes[entry].bounds);
            max_order = cmp::max(max_order, self.nodes[entry].max_order);
        }
        (bounds, max_order)
    }

    /// Splits overflowing `entries` in two at the axis and position with the
    /// smallest summed half-perimeter, each part keeping `MIN_CHILDREN`.
    fn split<'a>(
        &mut self,
        entries: &'a mut [usize; MAX_CHILDREN + 1],
    ) -> (&'a [usize], &'a [usize]) {
        let mut best: Option<(U, bool, usize)> = None;
        let mut by_y = *entries;
        for (vertical, order) in [(false, &mut *entries), (true, &mut by_y)] {
            self.sort_by_lower_edge(order, vertical);
            self.split_covers.clear();
            let mut suffix = self.nodes[order[MAX_CHILDREN]].bounds.clone();
            self.split_covers.push(suffix.clone());
            for &entry in order[..MAX_CHILDREN].iter().rev() {
                suffix = suffix.union(&self.nodes[entry].bounds);
                self.split_covers.push(suffix.clone());
            }
            // `split_covers[i]` covers the last `i + 1` entries.
            let mut prefix = self.nodes[order[0]].bounds.clone();
            for k in 1..=MAX_CHILDREN {
                if k >= MIN_CHILDREN && MAX_CHILDREN + 1 - k >= MIN_CHILDREN {
                    let cost = prefix.half_perimeter()
                        + self.split_covers[MAX_CHILDREN - k].half_perimeter();
                    if best.as_ref().is_none_or(|(c, _, _)| cost < *c) {
                        best = Some((cost, vertical, k));
                    }
                }
                prefix = prefix.union(&self.nodes[order[k]].bounds);
            }
        }
        let (_, vertical, k) = best.expect("a split position exists");
        if vertical {
            *entries = by_y;
        }
        let (left, right) = entries.split_at(k);
        (left, right)
    }

    /// Insertion sort by the lower edge on one axis: never panics on
    /// unordered values (NaN), unlike `sort_by` with a partial order.
    fn sort_by_lower_edge(&self, entries: &mut [usize], vertical: bool) {
        let edge = |entry: usize| {
            let origin = &self.nodes[entry].bounds.origin;
            if vertical {
                origin.y.clone()
            } else {
                origin.x.clone()
            }
        };
        for i in 1..entries.len() {
            let mut j = i;
            while j > 0 && edge(entries[j]) < edge(entries[j - 1]) {
                entries.swap(j, j - 1);
                j -= 1;
            }
        }
    }
}

impl<U> Default for BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    fn default() -> Self {
        BoundsTree {
            nodes: Vec::new(),
            root: None,
            max_leaf: None,
            insert_path: Vec::new(),
            search_stack: Vec::new(),
            height: 0,
            split_covers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, Point, Size};
    use rand::{Rng, SeedableRng};

    #[test]
    fn test_insert() {
        let mut tree = BoundsTree::<f32>::default();
        let bounds1 = Bounds {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds2 = Bounds {
            origin: Point { x: 5.0, y: 5.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds3 = Bounds {
            origin: Point { x: 10.0, y: 10.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };

        // Insert the bounds into the tree and verify the order is correct
        assert_eq!(tree.insert(bounds1), 1);
        assert_eq!(tree.insert(bounds2), 2);
        assert_eq!(tree.insert(bounds3), 3);

        // Insert non-overlapping bounds and verify they can reuse orders
        let bounds4 = Bounds {
            origin: Point { x: 20.0, y: 20.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds5 = Bounds {
            origin: Point { x: 40.0, y: 40.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds6 = Bounds {
            origin: Point { x: 25.0, y: 25.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        assert_eq!(tree.insert(bounds4), 1); // bounds4 does not overlap with bounds1, bounds2, or bounds3
        assert_eq!(tree.insert(bounds5), 1); // bounds5 does not overlap with any other bounds
        assert_eq!(tree.insert(bounds6), 2); // bounds6 overlaps with bounds4, so it should have a different order
    }

    #[test]
    fn test_random_iterations() {
        let max_bounds = 100;
        for seed in 1..=1000 {
            // let seed = 44;
            let mut tree = BoundsTree::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);
            let mut expected_quads: Vec<(Bounds<f32>, u32)> = Vec::new();

            // Insert a random number of random AABBs into the tree.
            let num_bounds = rng.random_range(1..=max_bounds);
            for _ in 0..num_bounds {
                let min_x: f32 = rng.random_range(-100.0..100.0);
                let min_y: f32 = rng.random_range(-100.0..100.0);
                let width: f32 = rng.random_range(0.0..50.0);
                let height: f32 = rng.random_range(0.0..50.0);
                let bounds = Bounds {
                    origin: Point { x: min_x, y: min_y },
                    size: Size { width, height },
                };

                let expected_ordering = expected_quads
                    .iter()
                    .filter_map(|quad| quad.0.intersects(&bounds).then_some(quad.1))
                    .max()
                    .unwrap_or(0)
                    + 1;
                expected_quads.push((bounds, expected_ordering));

                // Insert the AABB into the tree and collect intersections.
                let actual_ordering = tree.insert(bounds);
                assert_eq!(actual_ordering, expected_ordering);
            }
        }
    }

    /// Checks the balanced-tree invariants below `node` and returns the depth
    /// of its leaves.
    fn check_node(tree: &BoundsTree<f32>, node: usize) -> usize {
        match &tree.nodes[node].kind {
            NodeKind::Leaf { order } => {
                assert_eq!(*order, tree.nodes[node].max_order);
                0
            }
            NodeKind::Internal { children } => {
                let children = children.as_slice();
                assert!(!children.is_empty() && children.len() <= MAX_CHILDREN);
                let depths: Vec<usize> = children.iter().map(|&c| check_node(tree, c)).collect();
                assert!(
                    depths.iter().all(|&d| d == depths[0]),
                    "leaves at one depth"
                );
                let max = children
                    .iter()
                    .map(|&c| tree.nodes[c].max_order)
                    .max()
                    .unwrap();
                assert_eq!(tree.nodes[node].max_order, max);
                assert_eq!(
                    tree.nodes[*children.last().unwrap()].max_order,
                    max,
                    "max child last"
                );
                for &c in children {
                    let (outer, inner) = (&tree.nodes[node].bounds, &tree.nodes[c].bounds);
                    assert!(outer.origin.x <= inner.origin.x && outer.origin.y <= inner.origin.y);
                    assert!(outer.bottom_right().x >= inner.bottom_right().x);
                    assert!(outer.bottom_right().y >= inner.bottom_right().y);
                }
                depths[0] + 1
            }
        }
    }

    #[test]
    fn large_random_sets_split_nodes_and_keep_exact_orderings() {
        for seed in 1..=20u64 {
            let mut tree = BoundsTree::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut expected: Vec<(Bounds<f32>, u32)> = Vec::new();
            for _ in 0..1500 {
                let bounds = Bounds {
                    origin: Point {
                        x: rng.random_range(-500.0..500.0),
                        y: rng.random_range(-500.0..500.0),
                    },
                    size: Size {
                        width: rng.random_range(0.0..60.0),
                        height: rng.random_range(0.0..60.0),
                    },
                };
                let ordering = expected
                    .iter()
                    .filter_map(|(b, o)| b.intersects(&bounds).then_some(*o))
                    .max()
                    .unwrap_or(0)
                    + 1;
                expected.push((bounds, ordering));
                assert_eq!(tree.insert(bounds), ordering);
            }
            assert_eq!(check_node(&tree, tree.root.unwrap()), tree.height);
        }
    }

    #[test]
    fn document_order_text_stays_shallow() {
        // Glyph-sized boxes in reading order, the stream that made the old
        // descent nest pairs about 20 levels deep.
        let mut tree = BoundsTree::default();
        let mut count = 0usize;
        for line in 0..400 {
            for glyph in 0..60 {
                let bounds = Bounds {
                    origin: Point {
                        x: glyph as f32 * 6.0,
                        y: line as f32 * 18.0,
                    },
                    size: Size {
                        width: 6.0,
                        height: 18.0,
                    },
                };
                assert_eq!(tree.insert(bounds), 1);
                count += 1;
            }
        }
        assert_eq!(check_node(&tree, tree.root.unwrap()), tree.height);
        // A balanced tree with at least MIN_CHILDREN per split node.
        let bound = (count as f64).log(MIN_CHILDREN as f64).ceil() as usize + 1;
        assert!(tree.height <= bound, "height {} > {}", tree.height, bound);
    }
}
