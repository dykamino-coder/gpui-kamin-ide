//! Paint effects of the build contract: an effect around a box the algorithm
//! has built. Layout decides which effect a box needs and under which style;
//! paint's wrappers (`paint::effects`, `paint::decorations`) apply it.

use crate::paint::context::PaintCx;
use gpui::{Bounds, Pixels};
use std::cell::Cell;
use std::rc::Rc;

/// Union of unrounded boxes, filled on prepaint (see `Transformed::ref_box`).
pub(crate) type RefBox = Rc<Cell<Option<Bounds<Pixels>>>>;

/// A paint effect around a box the algorithm has built. The box keeps its
/// layout; the effect only changes how and when it is painted.
pub(crate) enum Effect<'a> {
    /// The outline phase (CSS 2.1 Appendix E, step 10) isolated inside the
    /// box: a float is painted atomically (E.2 step 5), so its outlines are
    /// painted with it rather than with the enclosing phase.
    OutlineScope(&'a Rc<PaintCx>),
    /// The outline phase of a formatting context, which is no stacking
    /// context: the enclosing phase is reused when one is collecting.
    OutlineFlow(&'a Rc<PaintCx>),
}
