//! What the build knows of the flow before a block: the least height of its
//! text, and whether a float of it may reach the block.

use super::{AREA_PER_CHAR, Kind, Slot, eligible};
use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, IntoElement};
use std::cell::Cell;
use std::rc::Rc;

/// The frame's own build runs (`FlowReuse::building`).
pub(crate) struct Building<'a>(&'a Cell<u32>);

impl Drop for Building<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

impl FlowReuse {
    /// The frame tree is being built while the guard lives. Only its blocks
    /// stand as leaves: a build later in the frame (a probe, a column copy, a
    /// nested scroller's content) lays out in its own pass, whose nodes the
    /// frame check could not read.
    pub(crate) fn building(&self) -> Building<'_> {
        let depth = &self.offscreen.building;
        depth.set(depth.get() + 1);
        Building(depth)
    }

    /// A paragraph is being built while the guard lives: the blocks of its
    /// atoms stand built, as a kept paragraph replays its atoms' paint.
    pub(crate) fn inline(&self) -> Building<'_> {
        let depth = &self.offscreen.inline;
        depth.set(depth.get() + 1);
        Building(depth)
    }

    /// Leaves this build stood so far: a block whose build stood one is not
    /// recorded for a replay (`Mark::leaves`).
    pub(crate) fn leaves_made(&self) -> u32 {
        self.offscreen.made.get()
    }

    /// The root scroller builds the document again (another scrollbar
    /// choice): only the last build's blocks are in the frame.
    fn restart(&self) -> Building<'_> {
        self.offscreen.slots.borrow_mut().clear();
        self.offscreen.above.set(0.0);
        self.offscreen.floated.set(false);
        self.building()
    }

    /// This build's own blocks under `opts` count (`building`).
    pub(super) fn counts(&self, opts: &RenderOpts) -> bool {
        opts.cx.contracts.document_root == Some(self.doc)
            && !opts.cx.contracts.offscreen_off
            && self.offscreen.building.get() > 0
            && self.offscreen.inline.get() == 0
    }

    /// Whether a float may reach the next block of the flow being built; a
    /// container's build restores it on its end (`restore_floated`), and its
    /// parent learns of the floats its block lets out (`built_block`).
    pub(crate) fn floated(&self) -> bool {
        self.offscreen.floated.get()
    }

    pub(crate) fn restore_floated(&self, floated: bool) {
        self.offscreen.floated.set(floated);
    }

    /// The least height of the flow built so far.
    pub(crate) fn above(&self) -> f32 {
        self.offscreen.above.get()
    }

    /// Node `n` of the flow was built: a float it lets out reaches the next
    /// blocks. A block built from `before` (`above`) pushes them down by its
    /// whole text, however its build reached the text; a scroller builds its
    /// content later and is as high as its own box.
    /// `zone` — the block's element, if one: a float it lets out is placed
    /// by its laid out box (`float_zone`), so only leaves it reaches are
    /// refused. Without one the next blocks of the flow stand built.
    pub(crate) fn built_block(
        &self,
        n: &Node,
        before: Option<f32>,
        zone: Option<&mut AnyElement>,
        opts: &RenderOpts,
    ) {
        let Node::Element(e) = n else { return };
        if !self.counts(opts) {
            return;
        }
        let layout = &e.style.used().reset_box_layout;
        let visible = |o| matches!(o, None | Some(crate::style::computed::Overflow::Visible));
        if let Some(before) = before.filter(|_| visible(layout.overflow_y)) {
            let mut chars = 0;
            super::super::plain::all_elements(e, |e| {
                chars += eligible::own_text(e);
                true
            });
            self.offscreen.above.set(before + height(chars, opts));
        }
        if self.offscreen.floated.get() || !super::super::plain::escaping_floats(e) {
            return;
        }
        match zone {
            Some(element) => {
                let slot = Rc::new(Slot::new(e.node_id, Kind::Floats, 0));
                super::super::push(&self.offscreen.slots, slot.clone());
                let child = std::mem::replace(element, gpui::Empty.into_any_element());
                *element = super::leaf::watch(slot, child);
            }
            None => self.offscreen.floated.set(true),
        }
    }

    /// An inline run `nodes` of this build: its text pushes later blocks down.
    pub(crate) fn note_run(&self, nodes: &[Node], opts: &RenderOpts) {
        fn chars(nodes: &[Node]) -> usize {
            let each = nodes.iter().map(|n| match n {
                Node::Text(t) => t.chars().filter(|c| !c.is_whitespace()).count(),
                Node::Element(e) => chars(&e.children),
            });
            each.sum()
        }
        if self.counts(opts) {
            let above = &self.offscreen.above;
            above.set(above.get() + height(chars(nodes), opts));
        }
    }
}

/// The build contract of a scroller's content (`tag` the scroller's), and,
/// for the root scroller, which builds the document in its own layout
/// request (`interactive::scroll_box`), the frame's build (`FlowReuse::building`).
pub(crate) fn root_scroller_build<'a>(
    opts: &'a RenderOpts,
    tag: &str,
) -> (
    crate::layout::build_contracts::ChildBuild,
    Option<Building<'a>>,
) {
    let reuse = opts.cx.contracts.reuse.as_ref();
    let root = reuse.filter(|_| tag == "html-viewport");
    (opts.cx.contracts.build, root.map(|r| r.restart()))
}

/// The least height `chars` characters of text take at the viewport's width.
pub(super) fn height(chars: usize, opts: &RenderOpts) -> f32 {
    chars as f32 * AREA_PER_CHAR / opts.viewport.0.max(1.0)
}
