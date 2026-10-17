//! Block axis of the page stack: pagination of a vertical root along x.
//!
//! css-page-3 §page-model: the page area is the fragmentainer, and content is
//! fragmented along the block axis of the root's writing mode (css-break-3 §2,
//! css-writing-modes-4 §3.1). The stack arithmetic (`ColumnStack::fill_at`, the
//! plan) stays logical — "height" is the block size — as in the vertical
//! multicol path (`column_stack/axis.rs`); this module converts to physical
//! boxes with `axis_box` (Blink `WritingModeConverter::ToPhysical`).

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::{StackAxis, axis_box};
use gpui::{Bounds, Pixels, point, px};

impl PageStack {
    /// Paginate along the block axis `axis` of the root writing mode.
    pub fn with_axis(mut self, axis: StackAxis) -> Self {
        self.axis = axis;
        self
    }

    /// (inline, block) size of the page area of `g`.
    pub(crate) fn area_logical(&self, g: &PageGeom) -> (f32, f32) {
        if self.axis.is_vertical() {
            (g.area.1, g.area.0)
        } else {
            g.area
        }
    }

    /// Physical box (unscaled stack points, relative to the sheet origin) of
    /// the block range `[bo, bo + be)` across the whole inline size of the
    /// page area of `g`.
    pub(crate) fn area_slice(&self, g: &PageGeom, bo: f32, be: f32) -> (f32, f32, f32, f32) {
        let (ax, ay) = g.area_origin();
        let (inline, block) = self.area_logical(g);
        let b: Bounds<Pixels> =
            axis_box(self.axis, point(px(ax), px(ay)), block, 0.0, inline, bo, be);
        (
            f32::from(b.origin.x),
            f32::from(b.origin.y),
            f32::from(b.size.width),
            f32::from(b.size.height),
        )
    }
}
