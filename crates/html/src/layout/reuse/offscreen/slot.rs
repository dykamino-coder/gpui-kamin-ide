//! A block of a build that stands, or could stand, as its leaf (`super`).

use gpui::{LayoutId, LayoutSnapshot};
use std::cell::{Cell, RefCell};

pub(super) enum Kind {
    Measured,
    Exact(LayoutSnapshot),
    Built,
    /// A built block whose floats reach past it: no leaf may meet them.
    Floats,
}

impl Slot {
    pub(super) fn new(node: u64, kind: Kind, at: u32) -> Self {
        Self {
            node,
            kind,
            at,
            layout: Cell::new(None),
            tree: Cell::new(None),
            measured: Cell::new(None),
            failed: Cell::new(false),
            sizes: Default::default(),
            builds: Cell::new(0),
        }
    }

    /// The block's layout node, in the tree being laid out now.
    pub(super) fn place(&self, id: LayoutId, window: &gpui::Window) {
        self.layout.set(Some(id));
        self.tree.set(Some(window.layout_tree_id()));
    }

    /// The block's node in window's frame tree.
    pub(super) fn node_in(&self, window: &gpui::Window) -> Option<LayoutId> {
        self.layout
            .get()
            .filter(|_| self.tree.get() == Some(window.layout_tree_id()))
    }

    /// A leaf stands for the block: the frame check decides on it.
    pub(super) fn leaf(&self) -> bool {
        matches!(self.kind, Kind::Measured | Kind::Exact(_))
    }
}

/// A block of this build standing as its leaf or built where a leaf could.
pub(crate) struct Slot {
    pub(super) node: u64,
    pub(super) kind: Kind,
    pub(super) at: u32,
    pub(super) layout: Cell<Option<LayoutId>>,
    /// The layout tree of `layout`: a block laid out in a nested engine (a
    /// measured host) is not in the frame's tree, which the check reads.
    pub(super) tree: Cell<Option<gpui::LayoutTreeId>>,
    /// The leaf of the block built by the measure at a definite width.
    pub(super) measured: Cell<Option<LayoutSnapshot>>,
    pub(super) failed: Cell<bool>,
    /// What the measure laid out at each width it was asked for.
    pub(super) sizes: RefCell<Vec<Laid>>,
    /// Builds of the block by its measure (`ReuseStats::offscreen_measures`).
    pub(super) builds: Cell<u32>,
}

/// A width a measure was asked for, its size and its exact leaf.
type Laid = (
    gpui::AvailableSpace,
    gpui::Size<gpui::Pixels>,
    Option<LayoutSnapshot>,
);
