//! CSS Overflow 3 §2.2: line boxes belong to scrollable overflow, unlike glyph ink.
use super::*;
use gpui::{Bounds, LayoutId, Pixels, Window, point, px, size};
impl Paragraph {
    pub(crate) fn scroll_owner(
        mut self,
        id: u64,
        sticky: Option<crate::interactive::scroll_extent::StickyShift>,
    ) -> Self {
        self.scroll_owner = Some((id, sticky));
        self
    }
    pub(super) fn publish_scroll_overflow(
        &self,
        bounds: Bounds<Pixels>,
        id: LayoutId,
        window: &mut Window,
    ) {
        let Some((owner, sticky)) = &self.scroll_owner else {
            return;
        };
        if !crate::interactive::scroll_extent::text_active() || self.paint_context.is_rotated() {
            return;
        }
        let exact = Bounds {
            origin: window.layout_origin_unrounded(id),
            size: window.layout_size_unrounded(id),
        };
        let half = 0.5 / window.scale_factor().max(0.01) + 1e-4;
        let near = |a: Pixels, b: Pixels| f32::from(a - b).abs() <= half;
        let b = if near(exact.left(), bounds.left()) && near(exact.top(), bounds.top()) {
            exact
        } else {
            bounds
        };
        let along = if self.vertical {
            b.size.height
        } else {
            b.size.width
        };
        let padding = self.line_padding();
        let mut block = px(0.0);
        for (row, line) in self.lines.iter().enumerate() {
            let (above, below) = padding.get(row).copied().unwrap_or_default();
            let thickness = self.line_height + px(above + below);
            let free = along - line.width - line.indent - px(self.flow_cut(row).1);
            let start = line_offset(self.line_align(row, line), self.wrap.rtl, free)
                + line.indent
                + px(self.flow_cut(row).0);
            let column = if self.lines_reversed {
                self.vertical_line_extent() - block - thickness
            } else {
                block
            };
            let rect = if !self.vertical {
                Bounds::new(
                    point(b.left() + start, b.top() + block),
                    size(line.width, thickness),
                )
            } else if self.vertical_ccw {
                Bounds::new(
                    point(b.left() + column, b.bottom() - start - line.width),
                    size(thickness, line.width),
                )
            } else {
                Bounds::new(
                    point(b.right() - column - thickness, b.top() + start),
                    size(thickness, line.width),
                )
            };
            crate::interactive::scroll_extent::record_text(*owner, rect, sticky.as_ref());
            block += thickness;
        }
    }
}
