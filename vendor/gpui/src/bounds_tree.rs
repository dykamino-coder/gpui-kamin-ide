use crate::{Bounds, Half, Point};
use std::{
    cmp,
    fmt::Debug,
    ops::{Add, Sub},
    rc::{Rc, Weak},
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
///
/// KaminIDE patch: a bottom entry can also be a *graft*: a whole frozen tree
/// shared by `Rc`, seen through a translation, an output clip and an ordering
/// offset (`Graft`). A scene that copies or replays recorded primitives
/// grafts the record's tree instead of inserting every primitive again;
/// queries through a graft stay exact (see `Graft`).
#[derive(Debug, Clone)]
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
    /// fast-path lookups). KaminIDE patch: `None` in a non-empty tree when
    /// the highest ordering belongs to a graft whose top rectangle is clipped
    /// away; the fast path is then off until a higher ordering is inserted.
    max_leaf: Option<(Bounds<U>, u32)>,
    /// Lowest ordering of any rectangle, grafts included.
    min_order: Option<u32>,
    /// Grafted trees; a bottom entry with `child == i + 1` stands for
    /// `grafts[i]`.
    grafts: Vec<Graft<U>>,
    /// Reusable descent path: node and the entry taken in it.
    insert_path: Vec<(usize, usize)>,
    /// Reusable stacks for search operations, one per graft nesting level.
    search_pool: Vec<Vec<usize>>,
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
    /// The child node; in bottom nodes 0 for a rectangle and `i + 1` for
    /// graft `i`.
    child: u32,
}

/// KaminIDE patch: a frozen tree grafted into another one.
///
/// The frozen tree `tree` indexes rectangles `e` with orderings `o`. Seen
/// through the graft they are the rectangles `(e + offset) ∩ clip` with
/// orderings `o + order`: exactly what inserting copies of those primitives,
/// moved by `offset` and clipped by `clip`, would index. A query `q` meets
/// `(e + offset) ∩ clip` iff `q ∩ clip` meets `e + offset`, so a query
/// through the graft clips itself once and walks the frozen tree unchanged.
#[derive(Debug, Clone)]
pub(crate) struct Graft<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    tree: Rc<BoundsTree<U>>,
    offset: Point<U>,
    clip: Option<Bounds<U>>,
    order: u32,
    /// Cover of the visible rectangles, in the grafting tree's space.
    cover: Bounds<U>,
    min: u32,
    max: u32,
}

fn moved<U>(bounds: &Bounds<U>, offset: Option<&Point<U>>) -> Bounds<U>
where
    U: Clone + Debug + Default + PartialEq + Add<U, Output = U>,
{
    match offset {
        None => bounds.clone(),
        Some(d) => Bounds {
            origin: Point {
                x: bounds.origin.x.clone() + d.x.clone(),
                y: bounds.origin.y.clone() + d.y.clone(),
            },
            size: bounds.size.clone(),
        },
    }
}

fn add_points<U>(a: &Point<U>, b: &Point<U>) -> Point<U>
where
    U: Clone + Debug + Default + PartialEq + Add<U, Output = U>,
{
    Point {
        x: a.x.clone() + b.x.clone(),
        y: a.y.clone() + b.y.clone(),
    }
}

/// The common part of `a` and `b` when it has an area (open-interval
/// overlap, as in `Bounds::intersects`).
fn meet<U>(a: &Bounds<U>, b: &Bounds<U>) -> Option<Bounds<U>>
where
    U: Clone + Debug + Default + PartialEq + PartialOrd + Add<U, Output = U> + Sub<Output = U>,
{
    let pick = |x: &U, y: &U, max: bool| {
        if (x < y) == max { y.clone() } else { x.clone() }
    };
    let x0 = pick(&a.origin.x, &b.origin.x, true);
    let y0 = pick(&a.origin.y, &b.origin.y, true);
    let a1 = a.bottom_right();
    let b1 = b.bottom_right();
    let x1 = pick(&a1.x, &b1.x, false);
    let y1 = pick(&a1.y, &b1.y, false);
    (x0 < x1 && y0 < y1).then(|| Bounds {
        origin: Point { x: x0.clone(), y: y0.clone() },
        size: crate::Size { width: x1 - x0, height: y1 - y0 },
    })
}

impl<U> Graft<U>
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
    /// The graft of `tree` moved by `offset`, clipped by `clip`, orderings
    /// raised by `order`; `None` when nothing of it is visible. A tree that
    /// is a single graft is seen through directly (offsets add, clips meet),
    /// so copies of copies do not deepen queries.
    pub fn new(
        mut tree: Rc<BoundsTree<U>>,
        mut offset: Point<U>,
        mut clip: Option<Bounds<U>>,
        mut order: u32,
    ) -> Option<Self> {
        while let Some(inner) = tree.single_graft() {
            let inner_clip = inner.clip.as_ref().map(|c| moved(c, Some(&offset)));
            clip = match (inner_clip, clip) {
                (Some(a), Some(b)) => Some(meet(&a, &b)?),
                (a, b) => a.or(b),
            };
            offset = add_points(&inner.offset, &offset);
            order += inner.order;
            let next = inner.tree.clone();
            tree = next;
        }
        let root = &tree.nodes[tree.root?];
        let cover = moved(&root.bounds, Some(&offset));
        let cover = match &clip {
            Some(c) => meet(&cover, c)?,
            None => cover,
        };
        let (min, max) = (tree.min_order? + order, root.max_order + order);
        Some(Self { tree, offset, clip, order, cover, min, max })
    }

    /// Cover of the visible rectangles.
    pub fn cover(&self) -> &Bounds<U> {
        &self.cover
    }

    /// Lowest ordering of the grafted rectangles.
    pub fn min_order(&self) -> u32 {
        self.min
    }

    /// Raise every grafted ordering by `by`.
    pub fn raise(&mut self, by: u32) {
        self.order += by;
        self.min += by;
        self.max += by;
    }

    /// KaminIDE patch: this graft without a share of its frozen tree, for an
    /// index that must not keep the tree's owner (a paint record) alive.
    pub fn downgrade(&self) -> WeakGraft<U> {
        WeakGraft {
            tree: Rc::downgrade(&self.tree),
            offset: self.offset.clone(),
            clip: self.clip.clone(),
            order: self.order,
            cover: self.cover.clone(),
            min: self.min,
            max: self.max,
        }
    }

    /// The highest ordering above `floor` of a grafted rectangle meeting
    /// `query`, in the space of a tree seen through `outer`; `floor` if none.
    fn max_above(
        &self,
        query: &Bounds<U>,
        outer: Option<&Point<U>>,
        floor: u32,
        pool: &mut Vec<Vec<usize>>,
        depth: usize,
    ) -> u32 {
        let offset = match outer {
            Some(o) => add_points(&self.offset, o),
            None => self.offset.clone(),
        };
        let clipped;
        let query = match &self.clip {
            Some(c) => match meet(query, &moved(c, outer)) {
                Some(q) => {
                    clipped = q;
                    &clipped
                }
                None => return floor,
            },
            None => query,
        };
        let inner_floor = floor.saturating_sub(self.order);
        let found = self
            .tree
            .max_above(query, Some(&offset), inner_floor, pool, depth);
        if found > inner_floor {
            cmp::max(floor, found + self.order)
        } else {
            floor
        }
    }
}

/// KaminIDE patch: a `Graft` whose frozen tree is held weakly. A frozen tree
/// is immutable while shared, so the graft's cover and orderings stay exact
/// for as long as the tree is alive.
#[derive(Debug, Clone)]
pub(crate) struct WeakGraft<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    tree: Weak<BoundsTree<U>>,
    offset: Point<U>,
    clip: Option<Bounds<U>>,
    order: u32,
    cover: Bounds<U>,
    min: u32,
    max: u32,
}

impl<U> WeakGraft<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// The graft again, if its tree is still alive.
    pub fn upgrade(&self) -> Option<Graft<U>> {
        Some(Graft {
            tree: self.tree.upgrade()?,
            offset: self.offset.clone(),
            clip: self.clip.clone(),
            order: self.order,
            cover: self.cover.clone(),
            min: self.min,
            max: self.max,
        })
    }
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
        self.min_order = None;
        self.grafts.clear();
        self.insert_path.clear();
        self.split_entries.clear();
        self.split_covers.clear();
    }

    /// KaminIDE patch: the graft this tree consists of, if it holds exactly
    /// one entry and that entry is a graft.
    fn single_graft(&self) -> Option<&Graft<U>> {
        let root = &self.nodes[self.root?];
        (self.height == 1 && root.len == 1 && root.entries[0].child > 0)
            .then(|| &self.grafts[root.entries[0].child as usize - 1])
    }

    /// KaminIDE patch: the highest ordering of any rectangle meeting `query`
    /// (0 if none), without inserting anything.
    pub fn max_meeting(&mut self, query: &Bounds<U>) -> u32 {
        self.find_max_ordering(query)
    }

    /// KaminIDE patch: add `graft`, whose orderings the caller already chose
    /// above everything it overlaps (`max_meeting` of its cover).
    pub fn insert_graft(&mut self, graft: Graft<U>) {
        let top = self.root.map_or(0, |root| self.nodes[root].max_order);
        if graft.max > top {
            // The frozen tree's top rectangle is the new top when visible.
            let inner_top = graft.tree.root.map_or(0, |r| graft.tree.nodes[r].max_order);
            self.max_leaf = graft.tree.max_leaf.as_ref().and_then(|(bounds, order)| {
                if *order != inner_top {
                    return None;
                }
                let bounds = moved(bounds, Some(&graft.offset));
                let bounds = match &graft.clip {
                    Some(c) => meet(&bounds, c)?,
                    None => bounds,
                };
                Some((bounds, graft.max))
            });
        }
        self.min_order = Some(self.min_order.map_or(graft.min, |m| m.min(graft.min)));
        let entry = Entry {
            bounds: graft.cover.clone(),
            max_order: graft.max,
            child: self.grafts.len() as u32 + 1,
        };
        self.grafts.push(graft);
        self.insert_entry(entry);
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
        let top = self.root.map_or(0, |root| self.nodes[root].max_order);
        if ordering > top {
            self.max_leaf = Some((new_bounds.clone(), ordering));
        }
        self.min_order = Some(self.min_order.map_or(ordering, |m| m.min(ordering)));

        self.insert_entry(Entry {
            bounds: new_bounds,
            max_order: ordering,
            child: 0,
        });
        ordering
    }

    /// Finds the maximum ordering among all bounds that intersect with the query.
    fn find_max_ordering(&mut self, query: &Bounds<U>) -> u32 {
        let mut pool = std::mem::take(&mut self.search_pool);
        let found = self.max_above(query, None, 0, &mut pool, 0);
        self.search_pool = pool;
        found
    }

    /// The highest ordering above `max_found` of a rectangle meeting `query`
    /// (`max_found` if none), with this tree's rectangles moved by `offset`.
    /// `pool[depth]` is the search stack of this nesting level.
    fn max_above(
        &self,
        query: &Bounds<U>,
        offset: Option<&Point<U>>,
        mut max_found: u32,
        pool: &mut Vec<Vec<usize>>,
        depth: usize,
    ) -> u32 {
        let Some(root_idx) = self.root else {
            return max_found;
        };
        if self.nodes[root_idx].max_order <= max_found {
            return max_found;
        }

        // Fast path: check if the max-ordering rectangle intersects
        if let Some((bounds, order)) = &self.max_leaf {
            if query.intersects(&moved(bounds, offset)) {
                return *order;
            }
        }

        // Slow path: search the tree
        if pool.len() <= depth {
            pool.resize_with(depth + 1, Vec::new);
        }
        let mut stack = std::mem::take(&mut pool[depth]);
        stack.clear();
        stack.push(root_idx);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            // Pruning: skip if this subtree can't improve our result
            if node.max_order <= max_found {
                continue;
            }

            if node.bottom {
                for entry in node.entries() {
                    if entry.max_order <= max_found
                        || !query.intersects(&moved(&entry.bounds, offset))
                    {
                        continue;
                    }
                    max_found = match entry.child {
                        0 => entry.max_order,
                        graft => self.grafts[graft as usize - 1]
                            .max_above(query, offset, max_found, pool, depth + 1),
                    };
                }
                continue;
            }

            // The child with the highest ordering is pushed last, so it is
            // searched first and prunes the most.
            let mut best: Option<&Entry<U>> = None;
            for entry in node.entries() {
                if entry.max_order <= max_found
                    || !query.intersects(&moved(&entry.bounds, offset))
                {
                    continue;
                }
                match best {
                    Some(b) if b.max_order >= entry.max_order => stack.push(entry.child as usize),
                    Some(b) => {
                        stack.push(b.child as usize);
                        best = Some(entry);
                    }
                    None => best = Some(entry),
                }
            }
            if let Some(best) = best {
                stack.push(best.child as usize);
            }
        }

        pool[depth] = stack;
        max_found
    }

    /// Inserts an entry of a bottom node and splits full nodes up to the root.
    fn insert_entry(&mut self, entry: Entry<U>) {
        let bounds = entry.bounds.clone();
        let order = entry.max_order;
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
            min_order: None,
            grafts: Vec::new(),
            insert_path: Vec::new(),
            search_pool: Vec::new(),
            split_entries: Vec::new(),
            split_covers: Vec::new(),
        }
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
    /// KaminIDE patch: a tree of `rects` with the given orderings, packed
    /// bottom-up by Sort-Tile-Recursive (Leutenegger, Edgington, Lopez 1997):
    /// sort by center x, cut into about sqrt(n / MAX_CHILDREN) vertical
    /// slices, sort each slice by center y and fill nodes in that order; repeat
    /// on the node covers. O(n log n) and fully packed nodes, against an
    /// incremental insert that descends and splits per rectangle. `key` maps a
    /// coordinate to a total order (NaN sorts last instead of panicking).
    /// KaminIDE patch: drop the spare capacity of a tree that is final (a
    /// paint record's or a copy's): such a tree is shared by the grafts of
    /// every scene that copies it and may outlive its record, so it keeps
    /// only its nodes and grafts, not growth room and scratch buffers.
    pub fn freeze(&mut self) {
        self.nodes.shrink_to_fit();
        self.grafts.shrink_to_fit();
        self.insert_path = Vec::new();
        self.search_pool = Vec::new();
        self.split_entries = Vec::new();
        self.split_covers = Vec::new();
    }

    pub fn packed(rects: Vec<(Bounds<U>, u32)>, key: impl Fn(&U) -> f32) -> Self {
        let mut tree = Self::default();
        let Some(top) = rects.iter().max_by_key(|(_, order)| *order).cloned() else {
            return tree;
        };
        tree.min_order = rects.iter().map(|(_, order)| *order).min();
        tree.max_leaf = Some(top);
        let mut level: Vec<Entry<U>> = rects
            .into_iter()
            .map(|(bounds, max_order)| Entry { bounds, max_order, child: 0 })
            .collect();
        let center = |entry: &Entry<U>, vertical: bool| {
            let (origin, size) = if vertical {
                (&entry.bounds.origin.y, &entry.bounds.size.height)
            } else {
                (&entry.bounds.origin.x, &entry.bounds.size.width)
            };
            key(origin) + key(size) * 0.5
        };
        let mut bottom = true;
        loop {
            let nodes = level.len().div_ceil(MAX_CHILDREN);
            let slices = (nodes as f64).sqrt().ceil().max(1.0) as usize;
            level.sort_by(|a, b| center(a, false).total_cmp(&center(b, false)));
            for slice in level.chunks_mut(slices * MAX_CHILDREN) {
                slice.sort_by(|a, b| center(a, true).total_cmp(&center(b, true)));
            }
            let mut next = Vec::with_capacity(nodes);
            for group in level.chunks(MAX_CHILDREN) {
                let mut node = Node::new(bottom);
                Self::fill(&mut node, group.iter().cloned());
                next.push(Entry {
                    bounds: node.bounds.clone(),
                    max_order: node.max_order,
                    child: tree.nodes.len() as u32,
                });
                tree.nodes.push(node);
            }
            tree.height += 1;
            bottom = false;
            if next.len() == 1 {
                tree.root = Some(next[0].child as usize);
                return tree;
            }
            level = next;
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

    fn random_rect(rng: &mut rand::rngs::StdRng, span: f32, size: f32) -> Bounds<f32> {
        Bounds {
            origin: Point {
                x: rng.random_range(-span..span),
                y: rng.random_range(-span..span),
            },
            size: Size {
                width: rng.random_range(0.5..size),
                height: rng.random_range(0.5..size),
            },
        }
    }

    fn brute_max(model: &[(Bounds<f32>, u32)], query: &Bounds<f32>) -> u32 {
        model
            .iter()
            .filter_map(|(b, o)| b.intersects(query).then_some(*o))
            .max()
            .unwrap_or(0)
    }

    /// Inserts `count` random rectangles, returning them with their orderings.
    fn filled(rng: &mut rand::rngs::StdRng, count: usize) -> (BoundsTree<f32>, Vec<(Bounds<f32>, u32)>) {
        let mut tree = BoundsTree::default();
        let mut model = Vec::new();
        for _ in 0..count {
            let rect = random_rect(rng, 300.0, 60.0);
            let order = tree.insert(rect);
            model.push((rect, order));
        }
        (tree, model)
    }

    #[test]
    fn frozen_tree_keeps_answers_without_spare_capacity() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let (mut tree, model) = filled(&mut rng, 37);
        let queries: Vec<_> = (0..50).map(|_| random_rect(&mut rng, 320.0, 40.0)).collect();
        let before: Vec<_> = queries.iter().map(|q| tree.max_meeting(q)).collect();
        tree.freeze();
        assert_eq!(tree.nodes.capacity(), tree.nodes.len());
        assert_eq!(tree.insert_path.capacity(), 0);
        assert_eq!(tree.split_entries.capacity(), 0);
        let after: Vec<_> = queries.iter().map(|q| tree.max_meeting(q)).collect();
        assert_eq!(before, after);
        for (query, answer) in queries.iter().zip(&after) {
            assert_eq!(*answer, brute_max(&model, query));
        }
        // A frozen tree still takes inserts exactly.
        let extra = random_rect(&mut rng, 320.0, 40.0);
        let order = tree.insert(extra);
        assert_eq!(order, brute_max(&model, &extra) + 1);
    }

    #[test]
    fn packed_tree_answers_like_the_inserted_one() {
        for seed in 1..=10u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let (mut inserted, model) = filled(&mut rng, 700);
            let mut packed = BoundsTree::packed(model.clone(), |x| *x);
            assert_eq!(check_node(&packed, packed.root.unwrap()), packed.height);
            assert_eq!(packed.min_order, inserted.min_order);
            for _ in 0..300 {
                let query = random_rect(&mut rng, 320.0, 40.0);
                let expected = brute_max(&model, &query);
                assert_eq!(packed.max_meeting(&query), expected);
                assert_eq!(inserted.max_meeting(&query), expected);
            }
        }
    }

    /// Grafts `source` (with orderings `source_model`) into `dest` the way a
    /// scene copies a record: moved, clipped and raised above what it covers.
    /// Returns the grafted rectangles as a fresh insert would index them.
    fn graft_into(
        dest: &mut BoundsTree<f32>,
        source: Rc<BoundsTree<f32>>,
        source_model: &[(Bounds<f32>, u32)],
        offset: Point<f32>,
        clip: Bounds<f32>,
    ) -> Vec<(Bounds<f32>, u32)> {
        let Some(mut graft) = Graft::new(source, offset, Some(clip), 0) else {
            return Vec::new();
        };
        let above = dest.max_meeting(graft.cover());
        let raise = if above < graft.min_order() { 0 } else { above + 1 - graft.min_order() };
        graft.raise(raise);
        dest.insert_graft(graft);
        source_model
            .iter()
            .filter_map(|(b, o)| meet(&moved(b, Some(&offset)), &clip).map(|b| (b, o + raise)))
            .collect()
    }

    #[test]
    fn grafts_answer_like_reinserted_copies() {
        for seed in 1..=12u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let (source, source_model) = filled(&mut rng, 400);
            let source = Rc::new(source);
            // A copy of a copy: a tree that is one graft, grafted again.
            let first_offset = Point { x: 37.25, y: -12.5 };
            let first_clip = random_rect(&mut rng, 100.0, 500.0);
            let mut copy = BoundsTree::default();
            let copy_model =
                graft_into(&mut copy, source.clone(), &source_model, first_offset, first_clip);
            let copy = Rc::new(copy);

            let (mut dest, mut model) = filled(&mut rng, 150);
            let before = model.clone();
            let offset = Point { x: -20.0, y: 55.5 };
            let clip = random_rect(&mut rng, 100.0, 500.0);
            let grafted = if seed % 2 == 0 {
                graft_into(&mut dest, copy.clone(), &copy_model, offset, clip)
            } else {
                graft_into(&mut dest, source.clone(), &source_model, offset, clip)
            };
            if seed % 2 == 0 && copy.root.is_some() {
                // The copy collapsed: one level of grafts below `dest`.
                assert!(dest.grafts.iter().all(|g| g.tree.grafts.is_empty()));
            }
            // Earlier overlapping rectangles stay below every grafted one.
            for (b, o) in &grafted {
                assert!(brute_max(&before, b) < *o);
            }
            model.extend(grafted);
            for _ in 0..300 {
                let query = random_rect(&mut rng, 320.0, 40.0);
                assert_eq!(dest.max_meeting(&query), brute_max(&model, &query));
            }
            // Later inserts see the grafted rectangles exactly.
            for _ in 0..200 {
                let rect = random_rect(&mut rng, 320.0, 60.0);
                let expected = brute_max(&model, &rect) + 1;
                assert_eq!(dest.insert(rect), expected);
                model.push((rect, expected));
            }
        }
    }

    #[test]
    fn unchanged_regraft_keeps_orderings() {
        // Nothing earlier overlaps above the graft's lowest ordering: no raise.
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let (source, _) = filled(&mut rng, 200);
        let mut graft = Graft::new(Rc::new(source), Point { x: 0.0, y: 0.0 }, None, 0).unwrap();
        let mut dest = BoundsTree::<f32>::default();
        assert_eq!(dest.max_meeting(graft.cover()), 0);
        graft.raise(0);
        let max = graft.max;
        dest.insert_graft(graft);
        assert_eq!(dest.root.map(|r| dest.nodes[r].max_order), Some(max));
        assert!(dest.max_leaf.is_some());
    }

    #[test]
    fn document_order_text_packs_shallow() {
        let mut rects = Vec::new();
        for line in 0..400 {
            for glyph in 0..60 {
                let origin = Point { x: glyph as f32 * 6.0, y: line as f32 * 18.0 };
                rects.push((Bounds { origin, size: Size { width: 6.0, height: 18.0 } }, 1));
            }
        }
        let count = rects.len();
        let tree = BoundsTree::packed(rects, |x| *x);
        assert_eq!(check_node(&tree, tree.root.unwrap()), tree.height);
        let bound = (count as f64).log(MIN_CHILDREN as f64).ceil() as usize + 1;
        assert!(tree.height <= bound, "height {} > {}", tree.height, bound);
    }
}
