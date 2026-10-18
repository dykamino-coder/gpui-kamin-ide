//! Layout inputs a box hands down to the boxes built inside it.
//!
//! Blink's `ConstraintSpace` / Servo's `ContainingBlock`: the parent decides
//! what its children may assume (containing-block width, available sizes) and
//! passes it with the build call. Here it rides in [`RenderOpts::cx`], which
//! already reaches every build function and is cloned into deferred build
//! closures — a closure sees the context of the place that created it.
//!
//! [`RenderOpts::cx`]: crate::render::RenderOpts::cx

use crate::layout::positioned::containing_block::Positioning;
use crate::render::RenderOpts;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct LayoutCx {
    /// Containing-block width in px when the parent's style gives it. A
    /// replaced element without intrinsic size but with a ratio takes its
    /// width from the block equation, i.e. from the containing block (CSS 2.1
    /// §10.3.2, last case); 300×150 applies only when there is none.
    pub(crate) cb_width: Option<f32>,
    /// Width a block-level child with `width: auto` gets in the current
    /// block container: its stretch-fit size (CSS 2.1 §10.3.3; css-sizing-3:
    /// the inline size of an in-flow block-level box is definite). Needed by
    /// `repeat(auto-fill | auto-fit, …)` (css-grid-1 §7.2.3.2) and by
    /// percentages of inline boxes, whose containing block is the paragraph's
    /// block. `None` — honestly unknown (flex, grid, table, inline-block,
    /// float, abspos, multicol, vertical writing).
    pub(crate) avail_w: Option<f32>,
    /// Definite content height of the current block container, for
    /// percentage heights of embedded viewports (CSS 2.1 §10.5).
    pub(crate) avail_h: Option<f32>,
    /// The block list being built is a table cell's (or caption's) own
    /// content: a BFC root whose floats it contains (§10.6.7).
    pub(crate) cell_bfc: bool,
    /// Containing-block layers of the current build pass (`Positioning`):
    /// a new document (`render`, a nested document) starts its own.
    pub(crate) positioning: Rc<Positioning>,
    /// Line-box hand-off for the static-position probe of the inline atom
    /// being built (`LineBoxSlot`); `None` outside such an atom.
    pub(crate) line_box: Option<crate::layout::positioned::containing_block::LineBoxSlot>,
}

impl RenderOpts {
    /// Options for the boxes inside the current one: same document options,
    /// context changed by `f`. Boxed — the build recursion is deep and its
    /// stack frames are tight.
    pub(crate) fn inner(&self, f: impl FnOnce(&mut LayoutCx)) -> Box<RenderOpts> {
        let mut opts = Box::new(self.clone());
        f(&mut opts.cx);
        opts
    }
}
