//! KaminIDE patch: native overflow geometry for independently measured children.

use super::{LayoutId, TaffyLayoutEngine};
use crate::{Bounds, Pixels, point, size};

impl TaffyLayoutEngine {
    pub fn overflow_contribution(&self, id: LayoutId, scale: f32) -> Bounds<Pixels> {
        let node = id.into();
        let layout = self.taffy.unrounded_layout(node);
        let style = self.taffy.style(node).expect(super::EXPECT_MESSAGE);
        let rect = taffy::compute_scrollable_overflow_contribution(
            layout.relative_offset,
            layout.size,
            layout.scrollable_overflow_rect,
            style.overflow,
            style.contain,
            false,
        );
        Bounds {
            origin: point(Pixels(rect.left / scale), Pixels(rect.top / scale)),
            size: size(
                Pixels((rect.right - rect.left) / scale),
                Pixels((rect.bottom - rect.top) / scale),
            ),
        }
    }
}
