//! KaminIDE patch: classic bars decided inside layout (CSS Overflow 3 §3, `overflow: auto`).
//!
//! A box whose bars layout decides (`ScrollbarLayout::auto`) is laid out with the bars it
//! guesses; after its layout (out-of-flow boxes included) its scrollable overflow says which
//! bars it needs, and when that differs only this box is laid out again with them, before
//! its parent places it. As Blink `BlockNode::Layout` (`ComputeScrollbars` before and after,
//! `AddScrollbarFreeze`: a bar that appeared stays for the rest of the layout) and Gecko
//! `ScrollContainerFrame::ReflowContents` (guess, then `TryLayout` of the scrolled frame only).
//! The viewport publishes its gutter before laying its content out, so the boxes built for
//! another root gutter follow the one it decides (`ScrollbarLayout::root_gutter`).
use super::{NodeId, Style, TaffyView};
use crate::geometry::{Point, Rect, Size};
use crate::style::{Direction, Overflow};
use crate::tree::traits::LayoutPartialTreeExt;
use crate::tree::{LayoutInput, LayoutOutput, RunMode};
use crate::util::ResolveOrZero;

/// Overflow up to this far past the scrollport needs no bar (as the host's scroll range).
const FIT: f32 = 0.5;

/// What the layouts of one box learned of its bars in one layout of it.
#[derive(Default)]
pub(super) struct Settling {
    /// Bars that appeared in this layout; they stay (no bar flip-flop).
    appeared: Point<bool>,
}

/// The space the box's bars take: a vertical bar takes width, a horizontal one height.
pub(super) fn bar_size(style: &Style) -> Size<f32> {
    let bar = |overflow: Overflow| if overflow == Overflow::Scroll { style.scrollbar_width } else { 0.0 };
    Size { width: bar(style.overflow.y), height: bar(style.overflow.x) }
}

/// The gutter of the box's bars where block layout reserves it (inline-end, block-end).
fn gutter(style: &Style) -> Rect<f32> {
    let bar = bar_size(style);
    let rtl = style.direction == Direction::Rtl;
    Rect {
        left: if rtl { bar.width } else { 0.0 },
        right: if rtl { 0.0 } else { bar.width },
        top: 0.0,
        bottom: bar.height,
    }
}

impl<NodeContext, MeasureFunction> TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput,
{
    /// The viewport publishes the gutter its content is laid out with now.
    #[inline(always)]
    pub(super) fn kamin_publish_root_gutter(&mut self, node_id: NodeId) {
        let style = &self.taffy.nodes[node_id.into()].style;
        if style.kamin_scrollbars.viewport() {
            self.taffy.kamin_root_gutter = Some((gutter(style), style.scrollbar_width));
        }
    }

    /// After a layout of `node_id`: whether its content asks for other bars, which are
    /// then set for the layout again. Only a complete layout decides: an intrinsic
    /// query can skip placement or report overflow under provisional constraints.
    /// It reads the current gutter without publishing another scrollbar decision.
    /// A full layout's gutter change invalidates the affected ancestor measurements.
    #[inline(never)]
    pub(super) fn kamin_settle_scrollbars(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        output: &LayoutOutput,
        settling: &mut Settling,
    ) -> bool {
        let style = &self.taffy.nodes[node_id.into()].style;
        let auto = style.kamin_scrollbars.auto();
        if inputs.run_mode != RunMode::PerformLayout || !(auto.x || auto.y) {
            return false;
        }
        let border = style.border.resolve_or_zero(inputs.parent_size.width, |val, basis| self.calc(val, basis));
        let bar = bar_size(style);
        // The scrollport: the padding box less the gutter (CSS Overflow 3 §2.2). The
        // overflow rectangle is measured from its start corner, end padding included.
        let view = Size {
            width: output.size.width - border.left - border.right - bar.width,
            height: output.size.height - border.top - border.bottom - bar.height,
        };
        #[cfg(feature = "content_size")]
        let content = output.scrollable_overflow_rect;
        #[cfg(not(feature = "content_size"))]
        let content = Rect::ZERO;
        let open = view.width > 0.0 && view.height > 0.0;
        let need = Point { x: open && content.right > view.width + FIT, y: open && content.bottom > view.height + FIT };
        let current = Point { x: style.overflow.x == Overflow::Scroll, y: style.overflow.y == Overflow::Scroll };
        let keep = |axis_auto: bool, need: bool, appeared: bool, current: bool| {
            if !axis_auto {
                current
            } else {
                need || appeared
            }
        };
        let frozen = self.kamin_frozen_scrollbars(node_id, output.size);
        let next = Point {
            x: keep(auto.x, need.x, settling.appeared.x || frozen.x, current.x),
            y: keep(auto.y, need.y, settling.appeared.y || frozen.y, current.y),
        };
        if next == current {
            return false;
        }
        settling.appeared.x |= next.x && !current.x;
        settling.appeared.y |= next.y && !current.y;
        self.kamin_gutter_changed(node_id, next, output.size);
        let mode = |bar: bool| if bar { Overflow::Scroll } else { Overflow::Hidden };
        let node = &mut self.taffy.nodes[node_id.into()];
        // A style shared with other nodes is copied: only this box changes its bars.
        super::SharedStyle::make_mut(&mut node.style).overflow = Point { x: mode(next.x), y: mode(next.y) };
        // Results measured with the other bars must not answer the layout again.
        node.cache.clear();
        true
    }
}

#[cfg(test)]
#[path = "kamin_scrollbars_tests.rs"]
mod tests;
