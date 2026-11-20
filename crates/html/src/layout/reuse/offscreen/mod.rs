//! Blocks far from the screen kept as their exact layout leaf only.
//!
//! Blink, Gecko and Servo keep the layout of the whole document: Blink skips
//! work off screen only under `content-visibility: auto`, Gecko builds its
//! display list for the displayport but keeps every frame and text run. Here a
//! block far from the screen (`eligible`) stands in the frame tree as one leaf
//! of its exact size and margins (`gpui::LayoutSnapshot`); its elements, layout
//! nodes and shaped lines live only while it is measured. DOM and styles stay,
//! so a block that comes near the screen (scroll, a script asking for its
//! geometry) is built again as in its first frame.
//!
//! A block without a kept leaf is measured where its parent lays it out: the
//! leaf is a measured node with the block's margins, whose measure builds the
//! block alone in a private arena and layout engine at the width the parent
//! gives (`leaf::Measured`). Before prepaint the frame checks every leaf
//! (`frame`): one near the screen, or whose margins or height the build did
//! not repeat exactly, is refused, and the frame is built again with it built
//! (`FlowReuse::validate`). Exact leaves and the places of the built blocks
//! are kept (`Kept`) so later frames place far blocks without a measure and
//! build the ones the scroll brings near.

mod above;
pub(crate) use above::root_scroller_build;
mod eligible;
mod frame;
mod leaf;
mod measure;
mod slot;
use slot::Kind;
pub(crate) use slot::Slot;

use super::FlowReuse;
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, LayoutSnapshot};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Area a character of text covers at least, in px² (a narrow glyph of 14 px
/// text at a tight line height): the text before a block bounds its top.
const AREA_PER_CHAR: f32 = 110.0;
/// How far past the root scrollport a document records its frame (each side,
/// `document::retained`): blocks inside it are built.
const INTEREST: f32 = 4000.0;

/// What a frame learnt of a block that may stand as its leaf.
#[derive(Clone, Copy)]
pub(super) struct Kept {
    /// The exact leaf; `None` — the block does not fit one (margins
    /// through its edges, overflow, another build): it is always built.
    snapshot: Option<LayoutSnapshot>,
    /// `BoxTree::clock` of the build: valid while the box's epoch is not newer.
    at: u32,
    /// Top and bottom of its border box in the unscrolled document.
    span: (f32, f32),
    viewport: (f32, f32),
}

/// Kept leaves of a document between frames, by node.
#[derive(Default)]
pub(crate) struct Store {
    kept: RefCell<HashMap<u64, Kept>>,
    /// Leaves refused by the frame check: its rebuild builds them.
    refused: Cell<HashSet<u64>>,
    /// Exact leaves refused only for another width (a scrollbar came): the
    /// rebuild measures them again at the width it gives.
    remeasure: Cell<HashSet<u64>>,
    /// Blocks every check of this frame saw: they keep what was learnt.
    seen: Cell<HashSet<u64>>,
    /// No block of this document stands as its leaf (tests compare frames
    /// with leaves against frames without them).
    #[cfg(test)]
    off: Cell<bool>,
}

impl Store {
    fn off(&self) -> bool {
        #[cfg(test)]
        return self.off.get();
        #[cfg(not(test))]
        false
    }
}

#[cfg(test)]
impl super::Store {
    /// Build this document's frames without leaves.
    pub(crate) fn turn_off_offscreen_leaves(&self) {
        self.offscreen.off.set(true);
    }
}

/// The leaves of one build.
#[derive(Default)]
pub(crate) struct Frame {
    slots: RefCell<Vec<Rc<Slot>>>,
    /// The text of the flow before the block being built, as the least height
    /// it takes at the viewport's width.
    above: Cell<f32>,
    /// The root scroller's offset (down positive); `None` without one.
    scroll: Cell<Option<f32>>,
    viewport: Cell<(f32, f32)>,
    /// Depth of the frame's own build (`FlowReuse::building`).
    building: Cell<u32>,
    /// A float built before in the block formatting context may reach into
    /// the next blocks: their lines go around it, a leaf's would not.
    floated: Cell<bool>,
    /// Leaves stood so far (`leaves_made`).
    made: Cell<u32>,
    /// Depth of paragraph builds (`FlowReuse::inline`).
    inline: Cell<u32>,
}

/// The build of a block child: its leaf, or a watch around its built element.
pub(crate) enum Far {
    Leaf(AnyElement),
    Watch(Rc<Slot>),
}

impl FlowReuse {
    /// The root scroller's offset of this build.
    pub(crate) fn set_scroll(&self, scroll: Option<f32>) {
        self.offscreen.scroll.set(scroll);
    }

    fn zone(&self, opts: &RenderOpts) -> (f32, f32) {
        let height = opts.viewport.1;
        let scroll = self.offscreen.scroll.get();
        let margin = match scroll {
            Some(_) => INTEREST.max(height),
            None => height,
        };
        let top = scroll.unwrap_or(0.0);
        (top - margin, top + height + margin)
    }

    /// Block child `e` of container `inherited`: its leaf if it lies far from
    /// the screen, a watch to wrap its built element in, or nothing.
    /// `edge` — the first or last element of its container, whose lines the
    /// container may read: always built.
    pub(crate) fn offscreen_leaf(
        &self,
        e: &Element,
        edge: bool,
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Option<Far> {
        if edge
            || e.node_id == 0
            || !self.counts(opts)
            || self.offscreen.floated.get()
            || self.store.offscreen.off()
        {
            return None;
        }
        let chars = eligible::far_leaf_text(e)?;
        self.offscreen.viewport.set(opts.viewport);
        let at = {
            let boxes = opts.cx.doc().boxes.borrow();
            let id = boxes.box_of(e.node_id)?;
            let epoch = boxes.build_epoch(id)?;
            (epoch, boxes.clock())
        };
        let store = &self.store.offscreen;
        let kept = store.kept.borrow().get(&e.node_id).copied();
        let kept = kept.filter(|k| at.0 <= k.at && k.at <= at.1 && k.viewport == opts.viewport);
        let has = |set: &Cell<HashSet<u64>>| {
            let ids = set.take();
            let found = ids.contains(&e.node_id);
            set.set(ids);
            found
        };
        let (refused, remeasure) = (has(&store.refused), has(&store.remeasure));
        let height = above::height(chars, opts);
        let (top, bottom) = self.zone(opts);
        let slot = |kind| Rc::new(Slot::new(e.node_id, kind, at.1));
        // A far block of a screen or less is measured alone. A larger one is
        // built through, so its children stand as leaves of their own: a
        // measure holds one small block at a time, never a large one whole.
        // The measure lays the block out in a block container; under an
        // anonymous box (a float band host, a split inline) it stands built.
        let small = height <= opts.viewport.1 && inherited.self_node != 0;
        let leaf = match kept {
            // A build after a refused rebuild stands no leaf at all.
            _ if refused || !self.allow => None,
            _ if remeasure => small.then_some(Kind::Measured),
            Some(k) => {
                let near = k.span.1 >= top && k.span.0 <= bottom;
                k.snapshot.filter(|_| !near).map(Kind::Exact)
            }
            None if !self.again && small && self.offscreen.above.get() > bottom => {
                Some(Kind::Measured)
            }
            None => None,
        };
        let Some(kind) = leaf else {
            if kept.is_some() {
                // What was learnt stays while the block is built through.
                let mut seen = store.seen.take();
                seen.insert(e.node_id);
                store.seen.set(seen);
            }
            // A built block is watched for its place and its exact leaf: a
            // later frame that finds it far stands it as that leaf, however
            // large it is (its children's leaves then go).
            let slot = slot(Kind::Built);
            super::push(&self.offscreen.slots, slot.clone());
            return Some(Far::Watch(slot));
        };
        self.offscreen
            .above
            .set(self.offscreen.above.get() + height);
        self.offscreen.made.set(self.offscreen.made.get() + 1);
        let slot = slot(kind);
        super::push(&self.offscreen.slots, slot.clone());
        Some(Far::Leaf(leaf::element(slot, e, inherited, opts)))
    }
}

impl Far {
    /// Wrap the element just built from `start` in `out` by the watch, if the
    /// build gave one element.
    pub(crate) fn wrap(self, start: usize, out: &mut Vec<AnyElement>) {
        let Far::Watch(slot) = self else { return };
        if out.len() == start + 1 {
            let child = out.pop().expect("one element");
            out.push(leaf::watch(slot, child));
        }
    }
}
