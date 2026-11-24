//! KaminIDE patch: the overflow clip of a laid out node, from layout alone.

use super::*;

impl TaffyLayoutEngine {
    /// The clip a node's overflow puts on its descendants (`Style::overflow_mask`):
    /// its padding box, unbounded along a visible axis, each edge from the
    /// unrounded geometry snapped to the device pixel grid as an HTML box's
    /// clip is (`Style::overflow_mask_snapped`). Relative to the root of this
    /// engine; `None` when the node does not clip.
    pub fn overflow_clip(&mut self, id: LayoutId, scale_factor: f32) -> Option<Bounds<Pixels>> {
        let style = self.taffy.style(id.into()).ok()?;
        let visible = |o| o == taffy::style::Overflow::Visible;
        let (vx, vy) = (visible(style.overflow.x), visible(style.overflow.y));
        if vx && vy {
            return None;
        }
        let border = self.taffy.layout(id.into()).ok()?.border;
        let origin = self.layout_origin_unrounded(id, scale_factor);
        let size = self.layout_size_unrounded(id, scale_factor);
        let at = |v: f32| Pixels((v * scale_factor).round() / scale_factor);
        let far = Pixels(1_000_000.0);
        let (x0, x1) = if vx {
            (at(-far.0), at(far.0))
        } else {
            let left = f32::from(origin.x) + border.left / scale_factor;
            let right = f32::from(origin.x + size.width) - border.right / scale_factor;
            (at(left), at(right))
        };
        let (y0, y1) = if vy {
            (at(-far.0), at(far.0))
        } else {
            let top = f32::from(origin.y) + border.top / scale_factor;
            let bottom = f32::from(origin.y + size.height) - border.bottom / scale_factor;
            (at(top), at(bottom))
        };
        Some(Bounds::from_corners(point(x0, y0), point(x1, y1)))
    }
}
