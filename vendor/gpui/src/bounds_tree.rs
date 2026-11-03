use crate::{Bounds, Half};
use std::{
    cmp,
    fmt::Debug,
    ops::{Add, Sub},
};

/// Maximum children per internal node (R-tree style branching factor).
/// Higher values = shorter tree = fewer cache misses, but more work per node.
const MAX_CHILDREN: usize = 12;

/// KaminIDE patch: minimum entries of a node made by a split (about 40% of
/// `MAX_CHILDREN`, as in the R*-tree).
const MIN_CHILDREN: usize = 4;

/// A spatial tree optimized for finding maximum ordering among intersecting bounds.
///
/// This is an R-tree variant specifically designed for the use case of assigning
/// z-order to overlapping UI elements. Key optimizations:
/// - Tracks the leaf with global max ordering for O(1) fast-path queries
/// - Uses higher branching factor (`MAX_CHILDREN`) for lower tree height
/// - Aggressive pruning during search based on max_order metadata
///
/// KaminIDE patch: a balanced R-tree whose nodes hold their entries inline.
/// Each entry is the bounds and the highest ordering of a child, or of an
/// inserted rectangle in a bottom node, so a descent or a query reads one
/// contiguous node instead of one node per child. The descent picks the
/// entry that needs the least enlargement (Guttman, "R-trees", SIGMOD 1984,
/// ChooseLeaf). A full node splits along the axis and position with the
/// smallest summed half-perimeter (the R*-tree split, Beckmann et al. 1990),
/// and splits propagate up to the root, so every bottom node has the same
/// depth. The previous tree never split a node: it nested a new pair under a
/// full parent, so document-order text built chains about 20 levels deep and
/// every insert scanned hundreds of children. Queries are exact, so the
/// assigned orderings are the same.
#[derive(Debug)]
pub(crate) struct BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// All nodes stored contiguously for cache efficiency.
    nodes: Vec<Node<U>>,
    /// Index of the root node, if any.
    root: Option<usize>,
    /// Node levels from the root to the bottom nodes (0 when empty).
    height: usize,
    /// Bounds and ordering of the rectangle with the highest ordering (for
    /// fast-path lookups).
    max_leaf: Option<(Bounds<U>, u32)>,
    /// Reusable descent path: node and the entry taken in it.
    insert_path: Vec<(usize, usize)>,
    /// Reusable stack for search operations.
    search_stack: Vec<usize>,
    /// Reusable entries of a node being split.
    split_entries: Vec<Entry<U>>,
    /// Reusable suffix covers of a node split.
    split_covers: Vec<Bounds<U>>,
}

/// One entry of a node: an inserted rectangle in a bottom node, a child node
/// otherwise.
#[derive(Debug, Clone, Default)]
struct Entry<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// The rectangle, or the cover of the child node.
    bounds: Bounds<U>,
    /// The ordering of the rectangle, or the highest one under the child.
    max_order: u32,
    /// The child node (unused in bottom nodes).
    child: u32,
}

/// A node of the bounds tree.
#[derive(Debug, Clone)]
struct Node<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// Bounding box containing every entry.
    bounds: Bounds<U>,
    /// Maximum ordering value in this subtree.
    max_order: u32,
    /// Whether the entries are inserted rectangles rather than child nodes.
    bottom: bool,
    len: u8,
    entries: [Entry<U>; MAX_CHILDREN],
}

impl<U> Node<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    fn new(bottom: bool) -> Self {
        Self {
            bounds: Bounds::default(),
            max_order: 0,
            bottom,
            len: 0,
            entries: std::array::from_fn(|_| Entry::default()),
        }
    }

    fn entries(&self) -> &[Entry<U>] {
        &self.entries[..self.len as usize]
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
        self.height = 0;
        self.max_leaf = None;
        self.insert_path.clear();
        self.search_stack.clear();
        self.split_entries.clear();
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

        // Update max_leaf tracking
        if self
            .max_leaf
            .as_ref()
            .is_none_or(|(_, order)| *order < ordering)
        {
            self.max_leaf = Some((new_bounds.clone(), ordering));
        }

        self.insert_entry(new_bounds, ordering);
        ordering
    }

    /// Finds the maximum ordering among all bounds that intersect with the query.
    fn find_max_ordering(&mut self, query: &Bounds<U>) -> u32 {
        let Some(root_idx) = self.root else {
            return 0;
        };

        // Fast path: check if the max-ordering rectangle intersects
        if let Some((bounds, order)) = &self.max_leaf {
            if query.intersects(bounds) {
                return *order;
            }
        }

        // Slow path: search the tree
        self.search_stack.clear();
        self.search_stack.push(root_idx);

        let mut max_found = 0u32;

        while let Some(node_idx) = self.search_stack.pop() {
            let node = &self.nodes[node_idx];

            // Pruning: skip if this subtree can't improve our result
            if node.max_order <= max_found {
                continue;
            }

            if node.bottom {
                for entry in node.entries() {
                    if entry.max_order > max_found && query.intersects(&entry.bounds) {
                        max_found = entry.max_order;
                    }
                }
                continue;
            }

            // The child with the highest ordering is pushed last, so it is
            // searched first and prunes the most.
            let mut best: Option<&Entry<U>> = None;
            for entry in node.entries() {
                if entry.max_order <= max_found || !query.intersects(&entry.bounds) {
                    continue;
                }
                match best {
                    Some(b) if b.max_order >= entry.max_order => {
                        self.search_stack.push(entry.child as usize)
                    }
                    Some(b) => {
                        self.search_stack.push(b.child as usize);
                        best = Some(entry);
                    }
                    None => best = Some(entry),
                }
            }
            if let Some(best) = best {
                self.search_stack.push(best.child as usize);
            }
        }

        max_found
    }

    /// Inserts a rectangle with the given ordering into a bottom node and
    /// splits full nodes up to the root.
    fn insert_entry(&mut self, bounds: Bounds<U>, order: u32) {
        let entry = Entry {
            bounds: bounds.clone(),
            max_order: order,
            child: 0,
        };
        let Some(root) = self.root else {
            let mut node = Node::new(true);
            node.bounds = bounds;
            node.max_order = order;
            node.entries[0] = entry;
            node.len = 1;
            self.nodes.push(node);
            self.root = Some(self.nodes.len() - 1);
            self.height = 1;
            return;
        };

        // Descend to the bottom node.
        self.insert_path.clear();
        let mut current = root;
        for _ in 1..self.height {
            let pos = self.choose_entry(current, &bounds);
            self.insert_path.push((current, pos));
            current = self.nodes[current].entries[pos].child as usize;
        }

        // Add bottom-up: a full node splits and hands its new sibling up.
        let mut split = self.push_entry(current, entry);
        let mut child = current;
        for level in (0..self.insert_path.len()).rev() {
            let (parent, pos) = self.insert_path[level];
            let (child_bounds, child_max) = {
                let child = &self.nodes[child];
                (child.bounds.clone(), child.max_order)
            };
            let node = &mut self.nodes[parent];
            node.entries[pos].bounds = child_bounds;
            node.entries[pos].max_order = child_max;
            node.bounds = node.bounds.union(&bounds);
            node.max_order = cmp::max(node.max_order, order);
            if let Some(sibling) = split.take() {
                split = self.push_entry(parent, sibling);
            }
            child = parent;
        }

        // The root split: a new root holds both halves.
        if let Some(sibling) = split {
            let old = &self.nodes[root];
            let mut node = Node::new(false);
            node.bounds = old.bounds.union(&sibling.bounds);
            node.max_order = cmp::max(old.max_order, sibling.max_order);
            node.entries[0] = Entry {
                bounds: old.bounds.clone(),
                max_order: old.max_order,
                child: root as u32,
            };
            node.entries[1] = sibling;
            node.len = 2;
            self.nodes.push(node);
            self.root = Some(self.nodes.len() - 1);
            self.height += 1;
        }
    }

    /// The entry of `node` that grows least to cover `bounds` (ties: the smaller).
    fn choose_entry(&self, node: usize, bounds: &Bounds<U>) -> usize {
        let mut best = 0;
        let mut best_cost = None::<(U, U)>;
        for (pos, entry) in self.nodes[node].entries().iter().enumerate() {
            let own = entry.bounds.half_perimeter();
            let grown = bounds.union(&entry.bounds).half_perimeter();
            let enlargement = grown - own.clone();
            let better = match &best_cost {
                None => true,
                Some((best_enlargement, best_size)) => {
                    enlargement < *best_enlargement
                        || (enlargement == *best_enlargement && own < *best_size)
                }
            };
            if better {
                best = pos;
                best_cost = Some((enlargement, own));
            }
        }
        best
    }

    /// Adds `entry` to `node`, or splits a full `node` and returns the entry
    /// of the new sibling that holds part of its entries.
    fn push_entry(&mut self, node: usize, entry: Entry<U>) -> Option<Entry<U>> {
        let n = &mut self.nodes[node];
        if (n.len as usize) < MAX_CHILDREN {
            n.bounds = n.bounds.union(&entry.bounds);
            n.max_order = cmp::max(n.max_order, entry.max_order);
            n.entries[n.len as usize] = entry;
            n.len += 1;
            return None;
        }
        let bottom = n.bottom;
        self.split_entries.clear();
        self.split_entries
            .extend(n.entries.iter_mut().map(std::mem::take));
        self.split_entries.push(entry);
        let k = self.split_position();

        let mut sibling = Node::new(bottom);
        Self::fill(&mut sibling, self.split_entries.drain(k..));
        let n = &mut self.nodes[node];
        n.len = 0;
        Self::fill(n, self.split_entries.drain(..));
        let sibling_entry = Entry {
            bounds: sibling.bounds.clone(),
            max_order: sibling.max_order,
            child: self.nodes.len() as u32,
        };
        self.nodes.push(sibling);
        Some(sibling_entry)
    }

    /// Fills an empty `node` with `entries` and sets its cover and ordering.
    fn fill(node: &mut Node<U>, entries: impl Iterator<Item = Entry<U>>) {
        node.len = 0;
        node.max_order = 0;
        for entry in entries {
            node.bounds = if node.len == 0 {
                entry.bounds.clone()
            } else {
                node.bounds.union(&entry.bounds)
            };
            node.max_order = cmp::max(node.max_order, entry.max_order);
            node.entries[node.len as usize] = entry;
            node.len += 1;
        }
    }

    /// Orders `split_entries` along the axis whose split at some position
    /// has the smallest summed half-perimeter, each part keeping
    /// `MIN_CHILDREN`, and returns that position.
    fn split_position(&mut self) -> usize {
        let mut best: Option<(U, bool, usize)> = None;
        for vertical in [false, true] {
            Self::sort_by_lower_edge(&mut self.split_entries, vertical);
            let entries = &self.split_entries;
            let len = entries.len();
            self.split_covers.clear();
            let mut suffix = entries[len - 1].bounds.clone();
            self.split_covers.push(suffix.clone());
            for entry in entries[..len - 1].iter().rev() {
                suffix = suffix.union(&entry.bounds);
                self.split_covers.push(suffix.clone());
            }
            // `split_covers[i]` covers the last `i + 1` entries.
            let mut prefix = entries[0].bounds.clone();
            for k in 1..len {
                if k >= MIN_CHILDREN && len - k >= MIN_CHILDREN {
                    let cost =
                        prefix.half_perimeter() + self.split_covers[len - 1 - k].half_perimeter();
                    if best.as_ref().is_none_or(|(c, _, _)| cost < *c) {
                        best = Some((cost, vertical, k));
                    }
                }
                prefix = prefix.union(&entries[k].bounds);
            }
        }
        let (_, vertical, k) = best.expect("a split position exists");
        if !vertical {
            Self::sort_by_lower_edge(&mut self.split_entries, false);
        }
        k
    }

    /// Insertion sort by the lower edge on one axis: never panics on
    /// unordered values (NaN), unlike `sort_by` with a partial order.
    fn sort_by_lower_edge(entries: &mut [Entry<U>], vertical: bool) {
        let edge = |entry: &Entry<U>| {
            if vertical {
                entry.bounds.origin.y.clone()
            } else {
                entry.bounds.origin.x.clone()
            }
        };
        for i in 1..entries.len() {
            let mut j = i;
            while j > 0 && edge(&entries[j]) < edge(&entries[j - 1]) {
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
            height: 0,
            max_leaf: None,
            insert_path: Vec::new(),
            search_stack: Vec::new(),
            split_entries: Vec::new(),
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

    /// Checks the balanced-tree invariants below `node` and returns the number
    /// of node levels down to its bottom nodes.
    fn check_node(tree: &BoundsTree<f32>, node: usize) -> usize {
        let n = &tree.nodes[node];
        let entries = n.entries();
        assert!(!entries.is_empty() && entries.len() <= MAX_CHILDREN);
        let max = entries.iter().map(|e| e.max_order).max().unwrap();
        assert_eq!(n.max_order, max);
        for e in entries {
            let (outer, inner) = (&n.bounds, &e.bounds);
            assert!(outer.origin.x <= inner.origin.x && outer.origin.y <= inner.origin.y);
            // `Bounds::union` stores `origin + size`, so the far edge
            // of a cover rounds by an ulp or so.
            assert!(outer.bottom_right().x >= inner.bottom_right().x - 1e-3);
            assert!(outer.bottom_right().y >= inner.bottom_right().y - 1e-3);
        }
        if n.bottom {
            return 1;
        }
        let depths: Vec<usize> = entries
            .iter()
            .map(|e| {
                let child = &tree.nodes[e.child as usize];
                assert_eq!(e.bounds, child.bounds);
                assert_eq!(e.max_order, child.max_order);
                check_node(tree, e.child as usize)
            })
            .collect();
        assert!(
            depths.iter().all(|&d| d == depths[0]),
            "bottom nodes at one depth"
        );
        depths[0] + 1
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
